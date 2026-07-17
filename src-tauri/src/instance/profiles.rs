use serde_json::Value;
use std::path::PathBuf;

#[tauri::command]
pub fn get_minecraft_default_path() -> String {
    if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
            .join(".minecraft")
            .to_string_lossy()
            .to_string()
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("Library/Application Support/minecraft")
            .to_string_lossy()
            .to_string()
    } else {
        PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".minecraft")
            .to_string_lossy()
            .to_string()
    }
}

#[tauri::command]
pub async fn load_profiles(base_dir: String) -> Result<Value, String> {
    let path = PathBuf::from(&base_dir).join("profiles.json");
    if !path.exists() {
        // Inicializa un JSON vacío la primera vez
        let default_profiles = serde_json::json!({});
        tokio::fs::write(&path, serde_json::to_string_pretty(&default_profiles).unwrap())
            .await
            .map_err(|e| e.to_string())?;
        return Ok(default_profiles);
    }
    let data = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile(base_dir: String, profile_id: String, profile_data: Value) -> Result<(), String> {
    let path = PathBuf::from(&base_dir).join("profiles.json");
    let mut profiles: Value = if path.exists() {
        let data = tokio::fs::read_to_string(&path).await.unwrap_or_default();
        serde_json::from_str(&data).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    profiles[&profile_id] = profile_data;
    tokio::fs::write(&path, serde_json::to_string_pretty(&profiles).unwrap())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}