use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::auth::models::AuthSession;
use crate::games::minecraft::arguments::extract_argument_list;
use crate::games::minecraft::assets::ensure_assets;
use crate::games::minecraft::classpath::{build_classpath, ensure_libraries};
use crate::games::minecraft::version::load_merged_version;
use crate::presence::{register_instance, unregister_instance, RunningInstance};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub profile_id: String,
    pub title: String,
    pub loader_name: String,
    pub instance_dir: String,
    pub version_id: String,
    pub java_path: String,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub extra_java_args: String,
}

#[tauri::command]
pub async fn cancel_preparation(profile_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let flags = state.preparing_cancel.lock().await;
    if let Some(flag) = flags.get(&profile_id) {
        flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub async fn launch_minecraft(
    window: tauri::Window,
    options: LaunchOptions,
    auth: AuthSession,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state.preparing_cancel.lock().await.insert(options.profile_id.clone(), cancel_flag.clone());

    macro_rules! bail_if_cancelled {
        () => {
            if cancel_flag.load(Ordering::SeqCst) {
                state.preparing_cancel.lock().await.remove(&options.profile_id);
                return Err("Cancelado por el usuario".to_string());
            }
        };
    }

    let instance_dir = PathBuf::from(&options.instance_dir);
    let version_json = load_merged_version(&instance_dir, &options.version_id).await?;

    bail_if_cancelled!();
    ensure_libraries(&instance_dir, &version_json, &cancel_flag).await?;

    bail_if_cancelled!();
    ensure_assets(&window, &instance_dir, &version_json, &cancel_flag).await?;

    bail_if_cancelled!();

    tokio::fs::create_dir_all(instance_dir.join("natives"))
        .await
        .map_err(|e| e.to_string())?;

    let classpath = build_classpath(&instance_dir, &version_json)?;
    let main_class = version_json["mainClass"]
        .as_str()
        .ok_or("mainClass no encontrado")?
        .to_string();
    let classpath_separator = if cfg!(target_os = "windows") { ";" } else { ":" };

    let mut vars = HashMap::new();
    vars.insert("auth_player_name".into(), auth.username.clone());
    vars.insert("version_name".into(), options.version_id.clone());
    vars.insert("game_directory".into(), instance_dir.to_string_lossy().to_string());
    vars.insert("assets_root".into(), instance_dir.join("assets").to_string_lossy().to_string());
    vars.insert(
        "assets_index_name".into(),
        version_json["assetIndex"]["id"].as_str().unwrap_or("legacy").to_string(),
    );
    vars.insert("auth_uuid".into(), auth.uuid.clone());
    vars.insert("auth_access_token".into(), auth.access_token.clone());
    vars.insert("clientid".into(), "0".into());
    vars.insert("auth_xuid".into(), "0".into());
    vars.insert("user_type".into(), auth.user_type.clone());
    vars.insert("version_type".into(), "release".into());
    vars.insert("natives_directory".into(), instance_dir.join("natives").to_string_lossy().to_string());
    vars.insert("launcher_name".into(), "LumineriaLauncher".into());
    vars.insert("launcher_version".into(), "1.0.0".into());
    vars.insert("classpath".into(), classpath);
    vars.insert("classpath_separator".into(), classpath_separator.to_string());
    vars.insert("library_directory".into(), instance_dir.join("libraries").to_string_lossy().to_string());

    let jvm_args = extract_argument_list(&version_json["arguments"]["jvm"], &vars);
    let game_args = extract_argument_list(&version_json["arguments"]["game"], &vars);

    bail_if_cancelled!();

    let mut command = Command::new(&options.java_path);
    command.current_dir(&instance_dir);
    command.env("LUMINERIA_IPC_PORT", state.ipc_port.to_string());
    command.env("LUMINERIA_PROFILE_ID", &options.profile_id);
    command.arg(format!("-Xms{}M", options.ram_min_mb));
    command.arg(format!("-Xmx{}M", options.ram_max_mb));

    for a in options.extra_java_args.split_whitespace() { command.arg(a); }
    for a in jvm_args { command.arg(a); }
    command.arg(&main_class);
    for a in game_args { command.arg(a); }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("No se pudo lanzar Java: {}", e))?;


    state.preparing_cancel.lock().await.remove(&options.profile_id);

    let profile_id = options.profile_id.clone();
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    state.running_processes.lock().await.insert(profile_id.clone(), tx);

    register_instance(
        &state.running_instances,
        &state.discord,
        RunningInstance {
            profile_id: profile_id.clone(),
            title: options.title.clone(),
            loader_name: options.loader_name.clone(),
            launched_at: crate::discord::now_ts(),
            ..Default::default()
        },
    ).await;

    let _ = window.emit("game-started", serde_json::json!({ "id": profile_id }));

    if let Some(stdout) = child.stdout.take() {
        let window_out = window.clone();
        let pid = profile_id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window_out.emit("game-log", serde_json::json!({ "id": pid, "line": line }));
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let window_err = window.clone();
        let pid = profile_id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window_err.emit("game-log", serde_json::json!({ "id": pid, "line": line }));
            }
        });
    }

    let window_exit = window.clone();
    let profile_id_exit = profile_id.clone();
    let state_ref = state.running_processes.clone();
    let running_instances_ref = state.running_instances.clone();
    let discord_ref = state.discord.clone();

    tokio::spawn(async move {
        tokio::select! {
            status = child.wait() => {
                if let Ok(st) = status {
                    if !st.success() {
                        let _ = window_exit.emit("game-exit-error", &format!("Minecraft cerró con error: {st}"));
                    }
                }
            },
            _ = &mut rx => {
                let _ = child.kill().await;
            }
        }

        state_ref.lock().await.remove(&profile_id_exit);
        unregister_instance(&running_instances_ref, &discord_ref, &profile_id_exit).await;
        let _ = window_exit.emit("game-stopped", serde_json::json!({ "id": profile_id_exit }));
    });

    Ok(())
}