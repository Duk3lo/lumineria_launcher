use std::path::PathBuf;

#[tauri::command]
pub async fn ensure_launcher_profile(instance_dir: String) -> Result<(), String> {
    let path = PathBuf::from(&instance_dir).join("launcher_profiles.json");
    if !path.exists() { 
        // FIX: Forge crashea si no existe la propiedad "profiles" adentro del json.
        tokio::fs::write(&path, r#"{"profiles":{}}"#).await.map_err(|e| e.to_string())?; 
    }
    Ok(())
}