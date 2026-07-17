use serde_json::Value;
use std::path::PathBuf;
use std::fs;

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

#[tauri::command]
pub async fn fetch_official_modpacks(base_dir: String) -> Result<Value, String> {
    let config_path = PathBuf::from(&base_dir).join("launcher_config.json");
    if !config_path.exists() {
        let default_config = serde_json::json!({
            "api_url": "http://localhost:8080/modpacks.json"
        });
        tokio::fs::write(&config_path, serde_json::to_string_pretty(&default_config).unwrap())
            .await
            .map_err(|e| e.to_string())?;
    }
    let config_data = tokio::fs::read_to_string(&config_path).await.map_err(|e| e.to_string())?;
    let config: Value = serde_json::from_str(&config_data).map_err(|e| e.to_string())?;
    let url = config["api_url"].as_str().unwrap_or("http://localhost:8080/modpacks.json");
    let response = reqwest::get(url).await.map_err(|e| format!("Error de conexión: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("El servidor respondió con error: {}", response.status()));
    }
    let data: Value = response.json().await.map_err(|e| format!("JSON inválido: {}", e))?;
    Ok(data)
}


#[tauri::command]
pub async fn get_installed_vanilla_versions() -> Result<Value, String> {
    let mc_path = PathBuf::from(get_minecraft_default_path()).join("versions");
    let mut installed = serde_json::json!({});

    if mc_path.exists() {
        if let Ok(entries) = fs::read_dir(mc_path) {
            for entry in entries.flatten() {
                let id = entry.file_name().to_string_lossy().to_string();
                let json_path = entry.path().join(format!("{}.json", id));
                
                if json_path.exists() {
                    installed[&id] = serde_json::json!({
                        "title": format!("Vanilla {}", id),
                        "mc_version": id,
                        "version_id": id,
                        "loader_name": "Vanilla",
                        "is_local": true 
                    });
                }
            }
        }
    }
    Ok(installed)
}

#[tauri::command]
pub async fn delete_profile(base_dir: String, profile_id: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(&base_dir).join("profiles.json");
    if path.exists() {
        let data = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
        let mut profiles: serde_json::Value = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        
        if let Some(obj) = profiles.as_object_mut() {
            obj.remove(&profile_id);
        }
        
        tokio::fs::write(&path, serde_json::to_string_pretty(&profiles).unwrap())
            .await
            .map_err(|e| e.to_string())?;
    }
    let instance_path = std::path::PathBuf::from(&base_dir).join("instances").join(&profile_id);
    if instance_path.exists() {
        tokio::fs::remove_dir_all(&instance_path).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}


#[tauri::command]
pub async fn fetch_neoforge_versions() -> Result<Vec<String>, String> {
    let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let response = reqwest::get(url).await.map_err(|e| format!("Error de conexión: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("NeoForge respondió con error: {}", response.status()));
    }
    let data: Value = response.json().await.map_err(|e| format!("JSON inválido: {}", e))?;
    let versions = data["versions"]
        .as_array()
        .ok_or("Formato inesperado en la respuesta de NeoForge")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    Ok(versions)
}

#[tauri::command]
pub async fn fetch_forge_versions() -> Result<Value, String> {
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
    let response = reqwest::get(url).await.map_err(|e| format!("Error de conexión: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Forge respondió con error: {}", response.status()));
    }
    let data: Value = response.json().await.map_err(|e| format!("JSON inválido: {}", e))?;
    Ok(data)
}