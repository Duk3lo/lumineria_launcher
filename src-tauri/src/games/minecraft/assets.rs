use futures_util::{StreamExt, TryStreamExt};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::time::{sleep, Duration};

use crate::net;

pub async fn ensure_assets(
    window: &tauri::Window,
    instance_dir: &Path,
    version_json: &Value,
    cancel: &Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let asset_id = version_json["assetIndex"]["id"].as_str().unwrap_or("legacy");
    let asset_url = version_json["assetIndex"]["url"].as_str();

    let indexes_dir = instance_dir.join("assets").join("indexes");
    tokio::fs::create_dir_all(&indexes_dir)
        .await
        .map_err(|e| e.to_string())?;
    let index_path = indexes_dir.join(format!("{}.json", asset_id));

    let index_json: Value = if index_path.exists() {
        let content = tokio::fs::read_to_string(&index_path)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| format!("JSON del índice corrupto: {}", e))?
    } else {
        let url = asset_url.ok_or("No se encontró la URL del assetIndex en el JSON de la versión")?;
        let resp = net::download_client()
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Sin conexión al descargar el índice de assets: {}", e))?;
        let raw = resp.text().await.map_err(|e| e.to_string())?;
        tokio::fs::write(&index_path, &raw)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())?
    };
    let objects_dir = instance_dir.join("assets").join("objects");
    let objects = index_json["objects"].as_object().cloned().unwrap_or_default();
    let mut pending = Vec::new();

    for meta in objects.values() {
        if let Some(hash) = meta["hash"].as_str() {
            let dest = objects_dir.join(&hash[0..2]).join(hash);
            if !dest.exists() {
                pending.push(hash.to_string());
            }
        }
    }

    let total = pending.len();
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicUsize::new(0));
    let objects_dir_shared = Arc::new(objects_dir);
    let cancel_shared = cancel.clone();

    futures_util::stream::iter(pending)
        .map(|hash| {
            let client = net::download_client().clone();
            let objects_dir = objects_dir_shared.clone();
            let done = done.clone();
            let window = window.clone();
            let cancel = cancel_shared.clone();

            async move {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("Cancelado por el usuario".to_string());
                }
                let prefix = &hash[0..2];
                let dest_dir = objects_dir.join(prefix);
                tokio::fs::create_dir_all(&dest_dir)
                    .await
                    .map_err(|e| e.to_string())?;
                let dest_file = dest_dir.join(&hash);

                let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);

                let mut success = false;
                let mut last_error = String::new();
                for _ in 0..3 {
                    match client.get(&url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(bytes) = resp.bytes().await {
                                if tokio::fs::write(&dest_file, &bytes).await.is_ok() {
                                    success = true;
                                    break;
                                }
                            }
                        }
                        Ok(resp) => last_error = format!("HTTP {}", resp.status()),
                        Err(e) => last_error = e.to_string(),
                    }
                    sleep(Duration::from_secs(1)).await;
                }

                if !success {
                    return Err(format!("Error descargando asset {}: {}", hash, last_error));
                }
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = window.emit(
                    "assets-progress",
                    serde_json::json!({ "done": n, "total": total }),
                );

                Ok::<(), String>(())
            }
        })
        .buffer_unordered(16)
        .try_for_each(|_| async { Ok(()) })
        .await?;

    Ok(())
}
