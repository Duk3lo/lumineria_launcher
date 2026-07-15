use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tauri::Emitter;

use crate::auth::models::AuthSession;
use crate::games::minecraft::version::load_merged_version;
use crate::games::minecraft::classpath::{ensure_libraries, build_classpath};
use crate::games::minecraft::assets::ensure_assets;
use crate::games::minecraft::arguments::extract_argument_list;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub instance_dir: String,
    pub version_id: String,
    pub java_path: String,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub extra_java_args: String,
}

#[tauri::command]
pub async fn launch_minecraft(window: tauri::Window, options: LaunchOptions, auth: AuthSession) -> Result<(), String> {
    let instance_dir = PathBuf::from(&options.instance_dir);
    let version_json = load_merged_version(&instance_dir, &options.version_id).await?;

    ensure_libraries(&instance_dir, &version_json).await?;
    ensure_assets(&window, &instance_dir, &version_json).await?;
    
    tokio::fs::create_dir_all(instance_dir.join("natives")).await.map_err(|e| e.to_string())?;

    let classpath = build_classpath(&instance_dir, &version_json, &options.version_id)?;
    let main_class = version_json["mainClass"].as_str().ok_or("mainClass no encontrado")?.to_string();
    let classpath_separator = if cfg!(target_os = "windows") { ";" } else { ":" };

    let mut vars = HashMap::new();
    vars.insert("auth_player_name".into(), auth.username.clone());
    vars.insert("version_name".into(), options.version_id.clone());
    vars.insert("game_directory".into(), instance_dir.to_string_lossy().to_string());
    vars.insert("assets_root".into(), instance_dir.join("assets").to_string_lossy().to_string());
    vars.insert("assets_index_name".into(), version_json["assetIndex"]["id"].as_str().unwrap_or("legacy").to_string());
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

    let mut command = Command::new(&options.java_path);
    command.current_dir(&instance_dir);
    command.arg(format!("-Xms{}M", options.ram_min_mb));
    command.arg(format!("-Xmx{}M", options.ram_max_mb));

    for a in options.extra_java_args.split_whitespace() { command.arg(a); }
    for a in jvm_args { command.arg(a); }
    command.arg(&main_class);
    for a in game_args { command.arg(a); }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("No se pudo lanzar Java: {}", e))?;

    if let Some(stdout) = child.stdout.take() {
        let window_out = window.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window_out.emit("game-log", &line);
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let window_err = window.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window_err.emit("game-log", &line);
            }
        });
    }

    let window_exit = window.clone();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) if !status.success() => {
                let _ = window_exit.emit("game-exit-error", &format!("Minecraft cerró con error: {status}"));
            },
            Err(e) => {
                let _ = window_exit.emit("game-exit-error", &format!("Error en proceso: {e}"));
            },
            _ => {}
        }
    });

    Ok(())
}