use std::path::Path;
use serde_json::Value;
use futures_util::{StreamExt, TryStreamExt};
use tauri::Emitter;

pub async fn ensure_assets(window: &tauri::Window, instance_dir: &Path, version_json: &Value) -> Result<(), String> {
    let asset_id = version_json["assetIndex"]["id"].as_str().unwrap_or("legacy");
    let asset_url = version_json["assetIndex"]["url"].as_str();

    let indexes_dir = instance_dir.join("assets").join("indexes");
    tokio::fs::create_dir_all(&indexes_dir).await.map_err(|e| e.to_string())?;
    let index_path = indexes_dir.join(format!("{}.json", asset_id));

    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap();

    let index_json: Value = if index_path.exists() {
        serde_json::from_str(&tokio::fs::read_to_string(&index_path).await.unwrap()).unwrap()
    } else {
        let url = asset_url.ok_or("Sin assetIndex.url")?;
        let raw = client.get(url).send().await.unwrap().text().await.unwrap();
        tokio::fs::write(&index_path, &raw).await.unwrap();
        serde_json::from_str(&raw).unwrap()
    };

    let objects_dir = instance_dir.join("assets").join("objects");
    let objects = index_json["objects"].as_object().cloned().unwrap_or_default();
    let mut pending = Vec::new();

    for meta in objects.values() {
        if let Some(hash) = meta["hash"].as_str() {
            let dest = objects_dir.join(&hash[0..2]).join(hash);
            if !dest.exists() { pending.push(hash.to_string()); }
        }
    }

    let total = pending.len();
    if total == 0 { return Ok(()); }

    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let objects_dir = std::sync::Arc::new(objects_dir);

    futures_util::stream::iter(pending).map(|hash| {
        let (client, objects_dir, done, window) = (client.clone(), objects_dir.clone(), done.clone(), window.clone());
        async move {
            let prefix = hash[0..2].to_string();
            let dest_dir = objects_dir.join(&prefix);
            tokio::fs::create_dir_all(&dest_dir).await.unwrap();
            let resp = client.get(&format!("https://resources.download.minecraft.net/{}/{}", prefix, hash)).send().await.unwrap();
            tokio::fs::write(dest_dir.join(&hash), &resp.bytes().await.unwrap()).await.unwrap();
            
            let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = window.emit("assets-progress", serde_json::json!({ "done": n, "total": total }));
            Ok::<(), String>(())
        }
    }).buffer_unordered(24).try_for_each(|_| async { Ok(()) }).await?;

    Ok(())
}