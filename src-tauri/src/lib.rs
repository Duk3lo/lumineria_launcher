mod java;
mod downloader;
mod games;
mod auth;
mod settings;
mod instance;

use tauri::Manager;

#[tauri::command]
fn get_default_path(app: tauri::AppHandle) -> String {
    app.path().app_local_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("LumineriaData"))
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
async fn ensure_dir(path: String) -> Result<(), String> {
    tokio::fs::create_dir_all(&path).await.map_err(|e| e.to_string())
}

/// Abre una carpeta (o archivo) con el explorador/gestor de archivos por defecto del sistema.
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Inicializa tus plugins aquí
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Nota: Si usas el nuevo opener de Tauri v2 puedes descomentar la siguiente línea y quitar el crate `open` externo
        // .plugin(tauri_plugin_opener::init()) 
        
        // Registra todos tus comandos
        .invoke_handler(tauri::generate_handler![
            get_default_path,
            ensure_dir,
            open_folder,
            downloader::ensure_launcher_profile,
            java::verify_and_get_java,
            java::download_java_command,
            downloader::download_generic_file,
            downloader::execute_jar,
            games::minecraft::launch_minecraft,
            games::minecraft::ensure_vanilla_version,
            auth::ms_login_start,
            auth::ms_login_poll,
            auth::offline_login,
            auth::save_session,
            auth::load_session,
            auth::clear_session,
            settings::load_settings,
            settings::save_settings,
            settings::get_system_ram_mb,
            instance::get_instance_status,
            instance::list_mods,
            instance::toggle_mod,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}