use serde_json::Value;
use std::path::PathBuf;

use crate::net;

#[tauri::command]
pub async fn ensure_fabric_profile(
    instance_dir: String,
    mc_version: String,
    loader_version: String,
) -> Result<String, String> {
    let version_id = format!("fabric-loader-{}-{}", loader_version, mc_version);
    let version_dir = PathBuf::from(&instance_dir).join("versions").join(&version_id);
    let json_path = version_dir.join(format!("{}.json", version_id));

    if json_path.exists() {
        return Ok(version_id);
    }

    tokio::fs::create_dir_all(&version_dir)
        .await
        .map_err(|e| e.to_string())?;

    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        mc_version, loader_version
    );

    let raw = net::http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Sin conexión al descargar el perfil de Fabric: {}", e))?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let _: Value = serde_json::from_str(&raw).map_err(|e| format!("Perfil de Fabric inválido: {}", e))?;

    tokio::fs::write(&json_path, &raw)
        .await
        .map_err(|e| e.to_string())?;

    Ok(version_id)
}
