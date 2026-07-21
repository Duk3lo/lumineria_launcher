use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub java_args_extra: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            ram_min_mb: 1024,
            ram_max_mb: 4096,
            java_args_extra: String::new(),
        }
    }
}

fn settings_path(base_dir: &str) -> PathBuf {
    PathBuf::from(base_dir).join("settings.json")
}

#[tauri::command]
pub async fn load_settings(base_dir: String) -> Result<LauncherSettings, String> {
    let path = settings_path(&base_dir);
    if !path.exists() {
        return Ok(LauncherSettings::default());
    }
    let raw = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_settings(base_dir: String, settings: LauncherSettings) -> Result<(), String> {
    let path = settings_path(&base_dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, raw).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_system_ram_mb() -> u64 {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}
