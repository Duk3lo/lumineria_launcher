#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::auth::models::AuthSession;

fn session_path(base_dir: &str) -> PathBuf {
    PathBuf::from(base_dir).join("session.json")
}

#[cfg(unix)]
async fn write_session_secure(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;

    let tmp_path = path.with_extension("json.tmp");

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp_path)
        .await?;

    file.write_all(data).await?;
    file.sync_all().await?;
    drop(file);

    tokio::fs::rename(&tmp_path, path).await?; // escritura atómica

    // Por si el archivo ya existía en disco con 644 de una versión anterior
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    Ok(())
}

#[cfg(not(unix))]
async fn write_session_secure(path: &Path, data: &[u8]) -> std::io::Result<()> {
    tokio::fs::write(path, data).await
}

#[tauri::command]
pub async fn save_session(base_dir: String, session: AuthSession) -> Result<(), String> {
    let path = session_path(&base_dir);
    if let Some(p) = path.parent() {
        tokio::fs::create_dir_all(p).await.map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    write_session_secure(&path, raw.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_session(base_dir: String) -> Result<Option<AuthSession>, String> {
    let path = session_path(&base_dir);
    if !path.exists() {
        return Ok(None);
    }
    #[cfg(unix)]
    {
        if let Ok(meta) = tokio::fs::metadata(&path).await {
            if meta.permissions().mode() & 0o777 != 0o600 {
                let _ = tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await;
            }
        }
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