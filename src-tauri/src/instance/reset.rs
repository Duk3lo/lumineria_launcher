use std::path::PathBuf;

#[tauri::command]
pub async fn reset_instance_libraries(instance_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(&instance_dir);
    for sub in ["libraries", "versions"] {
        let path = dir.join(sub);
        if path.exists() { tokio::fs::remove_dir_all(&path).await.map_err(|e| e.to_string())?; }
    }
    Ok(())
}