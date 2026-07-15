use std::path::PathBuf;
use serde_json::Value;

#[tauri::command]
pub async fn ensure_vanilla_version(instance_dir: String, mc_version: String) -> Result<(), String> {
    let version_dir = PathBuf::from(&instance_dir).join("versions").join(&mc_version);
    let json_path = version_dir.join(format!("{}.json", mc_version));
    let jar_path = version_dir.join(format!("{}.jar", mc_version));

    tokio::fs::create_dir_all(&version_dir).await.map_err(|e| e.to_string())?;

    let version_json: Value = if json_path.exists() {
        serde_json::from_str(&tokio::fs::read_to_string(&json_path).await.unwrap()).unwrap()
    } else {
        let manifest: Value = reqwest::get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
            .await.unwrap().json().await.unwrap();
        let entry_url = manifest["versions"].as_array().unwrap().iter()
            .find(|v| v["id"].as_str() == Some(&mc_version)).unwrap()["url"].as_str().unwrap();
        let raw = reqwest::get(entry_url).await.unwrap().text().await.unwrap();
        tokio::fs::write(&json_path, &raw).await.unwrap();
        serde_json::from_str(&raw).unwrap()
    };

    if !jar_path.exists() {
        if let Some(client_url) = version_json["downloads"]["client"]["url"].as_str() {
            let bytes = reqwest::get(client_url).await.unwrap().bytes().await.unwrap();
            tokio::fs::write(&jar_path, &bytes).await.unwrap();
        }
    }
    Ok(())
}