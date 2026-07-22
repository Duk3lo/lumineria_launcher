mod auth;
mod config;
mod discord;
mod downloader;
mod games;
mod instance;
mod ipc;
mod java;
mod net;
mod presence;
mod settings;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    pub running_processes: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    pub running_instances: Arc<Mutex<Vec<presence::RunningInstance>>>,
    pub discord: discord::DiscordHandle,
    pub ipc_port: u16,
    pub preparing_cancel: Arc<Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>>,
}

#[cfg(debug_assertions)]
fn prevent_default() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_prevent_default::Builder::new().build()
}

#[cfg(not(debug_assertions))]
fn prevent_default() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_prevent_default::Builder::new().build()
}

#[tauri::command]
fn get_default_path(app: tauri::AppHandle) -> String {
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap_or_default()
                .join("LumineriaData")
        })
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
async fn ensure_dir(path: String) -> Result<(), String> {
    tokio::fs::create_dir_all(&path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn kill_instance(
    profile_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut processes = state.running_processes.lock().await;
    if let Some(tx) = processes.remove(&profile_id) {
        let _ = tx.send(());
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();
    tauri::Builder::default()
        .plugin(prevent_default())
        .setup(|app| {
            let discord_handle = discord::spawn_discord_worker();
            let discord_for_ipc = discord_handle.clone();
            let running_instances = Arc::new(Mutex::new(Vec::new()));
            let running_instances_for_ipc = running_instances.clone();

            let ipc_port = tauri::async_runtime::block_on(async move {
                ipc::start_ipc_bridge(discord_for_ipc, running_instances_for_ipc).await
            })
            .expect("no se pudo iniciar el puente IPC local");

            discord_handle.send(discord::DiscordCommand::UpdateActivity {
                details: "En el launcher".into(),
                state: "Explorando modpacks".into(),
                large_image: Some("launcher_icon".into()),
                large_text: Some("Lumineria Launcher".into()),
                small_image: None,
                small_text: None,
                start_timestamp: Some(discord::now_ts()),
                party_size: None,
            });

            app.manage(AppState {
                running_processes: Arc::new(Mutex::new(HashMap::new())),
                running_instances,
                discord: discord_handle,
                ipc_port,
                preparing_cancel: Arc::new(Mutex::new(HashMap::new())),
            });
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            get_default_path,
            ensure_dir,
            open_folder,
            kill_instance,
            // --- Java ---
            java::verify_and_get_java,
            java::download_java_command,
            // --- Settings ---
            settings::load_settings,
            settings::save_settings,
            settings::get_system_ram_mb,
            // --- Downloader ---
            downloader::profile::ensure_launcher_profile,
            downloader::file::download_generic_file,
            downloader::jar::execute_jar,
            downloader::jar::check_version_installed,
            // --- Games / Minecraft ---
            games::minecraft::launcher::launch_minecraft,
            games::minecraft::vanilla::ensure_vanilla_version,
            games::minecraft::fabric::ensure_fabric_profile,
            games::minecraft::launcher::cancel_preparation,
            // --- Auth ---
            auth::microsoft::ms_login_start,
            auth::microsoft::ms_login_poll,
            auth::offline::offline_login,
            auth::session::save_session,
            auth::session::load_session,
            auth::session::clear_session,
            // --- Instance ---
            instance::status::get_instance_status,
            instance::mods::list_mods,
            instance::mods::toggle_mod,
            instance::mods::list_resource_packs,
            instance::mods::toggle_resource_pack,
            instance::reset::reset_instance_libraries,
            // --- Profiles ---
            instance::profiles::load_launcher_config,
            instance::profiles::save_launcher_config,
            instance::profiles::load_profiles,
            instance::profiles::save_profile,
            instance::profiles::delete_profile,
            instance::profiles::delete_vanilla_version,
            instance::profiles::get_installed_vanilla_versions,
            instance::profiles::get_minecraft_default_path,
            instance::profiles::fetch_official_modpacks,
            instance::profiles::fetch_neoforge_versions,
            instance::profiles::fetch_forge_versions,
            instance::profiles::cleanup_old_version,
            // --- Paquetes especiales ---
            instance::packwiz::sync_packwiz_modpack,
            // --- Red ---
            net::check_url_reachable,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
