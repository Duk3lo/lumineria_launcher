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

#[tauri::command]
pub async fn fetch_official_modpacks(base_dir: String) -> Result<Value, String> {
    // 1. Definimos la ruta del archivo de configuración
    let config_path = PathBuf::from(&base_dir).join("launcher_config.json");
    
    // 2. Si el archivo no existe, lo creamos con la IP de tu servidor en Ubuntu por defecto
    if !config_path.exists() {
        let default_config = serde_json::json!({
            "api_url": "http://localhost:8080/modpacks.json"
        });
        tokio::fs::write(&config_path, serde_json::to_string_pretty(&default_config).unwrap())
            .await
            .map_err(|e| e.to_string())?;
    }

    // 3. Leemos el archivo de configuración
    let config_data = tokio::fs::read_to_string(&config_path).await.map_err(|e| e.to_string())?;
    let config: Value = serde_json::from_str(&config_data).map_err(|e| e.to_string())?;
    
    // Extraemos la URL (si por alguna razón borran la línea, usamos localhost de respaldo)
    let url = config["api_url"].as_str().unwrap_or("http://localhost:8080/modpacks.json");

    // 4. Realizamos la petición a la URL que esté en el archivo
    let response = reqwest::get(url).await.map_err(|e| format!("Error de conexión: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("El servidor respondió con error: {}", response.status()));
    }

    let data: Value = response.json().await.map_err(|e| format!("JSON inválido: {}", e))?;
    Ok(data)
}