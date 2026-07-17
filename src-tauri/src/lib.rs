mod auth;
mod downloader;
mod games;
mod instance;
mod java;
mod settings;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    pub running_processes: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
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
    tauri::Builder::default()
        .manage(AppState {
            running_processes: Arc::new(Mutex::new(HashMap::new())),
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
            // --- Profiles (NUEVO: Lógica de base de datos) ---
            instance::profiles::load_profiles,
            instance::profiles::save_profile,
            instance::profiles::delete_profile,
            instance::profiles::get_installed_vanilla_versions,
            instance::profiles::get_minecraft_default_path,
            instance::profiles::fetch_official_modpacks,
            instance::profiles::fetch_neoforge_versions,
            instance::profiles::fetch_forge_versions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
