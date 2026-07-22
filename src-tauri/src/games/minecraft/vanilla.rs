use serde_json::Value;
use std::path::PathBuf;

use crate::net;

const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[tauri::command]
pub async fn ensure_vanilla_version(instance_dir: String, mc_version: String) -> Result<(), String> {
    let version_dir = PathBuf::from(&instance_dir).join("versions").join(&mc_version);
    let json_path = version_dir.join(format!("{}.json", mc_version));
    let jar_path = version_dir.join(format!("{}.jar", mc_version));

    tokio::fs::create_dir_all(&version_dir)
        .await
        .map_err(|e| e.to_string())?;

    let version_json: Value = if json_path.exists() {
        let raw = tokio::fs::read_to_string(&json_path)
            .await
            .map_err(|e| format!("No se pudo leer {}.json: {}", mc_version, e))?;
        serde_json::from_str(&raw).map_err(|e| format!("{}.json corrupto: {}", mc_version, e))?
    } else {
        let manifest_resp = net::http_client()
            .get(VERSION_MANIFEST_URL)
            .send()
            .await
            .map_err(|e| format!("Sin conexión al manifiesto de versiones: {}", e))?;
        let manifest: Value = manifest_resp
            .json()
            .await
            .map_err(|e| format!("Manifiesto de versiones inválido: {}", e))?;

        let entry_url = manifest["versions"]
            .as_array()
            .ok_or("Formato inesperado del manifiesto de versiones")?
            .iter()
            .find(|v| v["id"].as_str() == Some(mc_version.as_str()))
            .ok_or_else(|| format!("La versión {} no existe en el manifiesto", mc_version))?
            ["url"]
            .as_str()
            .ok_or("La entrada de versión no tiene URL")?
            .to_string();

        let raw = net::http_client()
            .get(&entry_url)
            .send()
            .await
            .map_err(|e| format!("Sin conexión al descargar {}.json: {}", mc_version, e))?
            .text()
            .await
            .map_err(|e| e.to_string())?;

        tokio::fs::write(&json_path, &raw)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::from_str(&raw).map_err(|e| format!("{}.json inválido: {}", mc_version, e))?
    };

    if !jar_path.exists() {
        if let Some(client_url) = version_json["downloads"]["client"]["url"].as_str() {
            let bytes = net::download_client()
                .get(client_url)
                .send()
                .await
                .map_err(|e| format!("Sin conexión al descargar {}.jar: {}", mc_version, e))?
                .bytes()
                .await
                .map_err(|e| e.to_string())?;

            tokio::fs::write(&jar_path, &bytes)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
