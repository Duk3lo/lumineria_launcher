use std::path::Path;
use serde_json::Value;
use futures_util::{StreamExt, TryStreamExt};
use tauri::Emitter;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{sleep, Duration};

pub async fn ensure_assets(window: &tauri::Window, instance_dir: &Path, version_json: &Value) -> Result<(), String> {
    // 1. Obtener ID y URL del Asset Index
    let asset_id = version_json["assetIndex"]["id"].as_str().unwrap_or("legacy");
    let asset_url = version_json["assetIndex"]["url"].as_str();

    let indexes_dir = instance_dir.join("assets").join("indexes");
    tokio::fs::create_dir_all(&indexes_dir).await.map_err(|e| e.to_string())?;
    let index_path = indexes_dir.join(format!("{}.json", asset_id));

    // 2. Configurar cliente HTTP con Timeout de 60 segundos
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {}", e))?;

    // 3. Cargar o descargar el archivo JSON del índice
    let index_json: Value = if index_path.exists() {
        let content = tokio::fs::read_to_string(&index_path).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| format!("JSON del índice corrupto: {}", e))?
    } else {
        let url = asset_url.ok_or("No se encontró la URL del assetIndex en el JSON de la versión")?;
        let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
        let raw = resp.text().await.map_err(|e| e.to_string())?;
        tokio::fs::write(&index_path, &raw).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())?
    };

    // 4. Identificar qué objetos faltan por descargar
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
    if total == 0 { return Ok(()); }

    // 5. Preparar descarga masiva controlada
    let done = Arc::new(AtomicUsize::new(0));
    let objects_dir_shared = Arc::new(objects_dir);

    futures_util::stream::iter(pending).map(|hash| {
        // Clonamos las referencias necesarias para cada hilo de descarga
        let client = client.clone();
        let objects_dir = objects_dir_shared.clone();
        let done = done.clone();
        let window = window.clone();

        async move {
            let prefix = &hash[0..2];
            let dest_dir = objects_dir.join(prefix);
            tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;
            let dest_file = dest_dir.join(&hash);

            let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);
            
            let mut success = false;
            let mut last_error = String::new();

            // Lógica de 3 REINTENTOS por cada archivo
            for _ in 0..3 {
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(bytes) = resp.bytes().await {
                            if tokio::fs::write(&dest_file, &bytes).await.is_ok() {
                                success = true;
                                break;
                            }
                        }
                    },
                    Ok(resp) => last_error = format!("HTTP {}", resp.status()),
                    Err(e) => last_error = e.to_string(),
                }
                // Esperar un segundo antes de reintentar si falló
                sleep(Duration::from_secs(1)).await;
            }

            if !success {
                return Err(format!("Error descargando asset {}: {}", hash, last_error));
            }
            
            // Actualizar progreso y enviar evento al frontend
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = window.emit("assets-progress", serde_json::json!({ "done": n, "total": total }));
            
            Ok::<(), String>(())
        }
    }).buffer_unordered(16) // Máximo 16 descargas simultáneas para no saturar la red
      .try_for_each(|_| async { Ok(()) }).await?;

    Ok(())
}