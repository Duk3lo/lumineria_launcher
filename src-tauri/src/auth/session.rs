use std::path::PathBuf;

use crate::auth::models::AuthSession;

fn session_path(base_dir: &str) -> PathBuf {
    PathBuf::from(base_dir).join("session.json")
}

#[tauri::command]
pub async fn save_session(base_dir: String, session: AuthSession) -> Result<(), String> {
    let path = session_path(&base_dir);
    if let Some(p) = path.parent() {
        tokio::fs::create_dir_all(p).await.map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, raw).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_session(base_dir: String) -> Result<Option<AuthSession>, String> {
    let path = session_path(&base_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&raw).ok())
}

#[tauri::command]
pub async fn clear_session(base_dir: String) -> Result<(), String> {
    let path = session_path(&base_dir);
    if path.exists() {
        tokio::fs::remove_file(&path).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
