use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct PackToml {
    #[allow(dead_code)]
    name: String,
    index: PackIndexRef,
}

#[derive(Debug, Deserialize)]
struct PackIndexRef {
    file: String,
}

#[derive(Debug, Deserialize)]
struct IndexToml {
    files: Vec<IndexFileEntry>,
}

#[derive(Debug, Deserialize)]
struct IndexFileEntry {
    file: String,
    #[serde(default)]
    metafile: bool,
}

#[derive(Debug, Deserialize)]
struct ModMetaToml {
    name: String,
    filename: String,
    #[serde(default)]
    side: Option<String>,
    download: ModDownload,
}

#[derive(Debug, Deserialize)]
struct ModDownload {
    url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SyncedMod {
    pub name: String,
    pub filename: String,
    pub status: String,
}

fn base_url(pack_url: &str) -> String {
    match pack_url.rfind('/') {
        Some(idx) => pack_url[..=idx].to_string(),
        None => pack_url.to_string(),
    }
}

fn resolve_url(base: &str, relative: &str) -> String {
    format!("{}{}", base, relative)
}

#[tauri::command]
pub async fn sync_packwiz_modpack(pack_url: String, instance_dir: String) -> Result<Vec<SyncedMod>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let base = base_url(&pack_url);

    let pack_raw = client.get(&pack_url).send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;
    let pack: PackToml = toml::from_str(&pack_raw).map_err(|e| format!("pack.toml inválido: {}", e))?;

    let index_url = resolve_url(&base, &pack.index.file);
    let index_raw = client.get(&index_url).send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;
    let index: IndexToml = toml::from_str(&index_raw).map_err(|e| format!("index.toml inválido: {}", e))?;

    let mut results = Vec::new();

    for entry in index.files.iter().filter(|f| f.metafile) {
        let meta_url = resolve_url(&base, &entry.file);

        let meta_raw = match client.get(&meta_url).send().await {
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(e) => {
                results.push(SyncedMod { name: entry.file.clone(), filename: entry.file.clone(), status: format!("error: {}", e) });
                continue;
            }
        };

        let meta: ModMetaToml = match toml::from_str(&meta_raw) {
            Ok(m) => m,
            Err(e) => {
                results.push(SyncedMod { name: entry.file.clone(), filename: entry.file.clone(), status: format!("error parseando: {}", e) });
                continue;
            }
        };

        if meta.side.as_deref() == Some("server") {
            results.push(SyncedMod { name: meta.name, filename: meta.filename, status: "omitido (server-only)".into() });
            continue;
        }

        let download_url = match &meta.download.url {
            Some(u) => u.clone(),
            None => {
                results.push(SyncedMod { name: meta.name, filename: meta.filename, status: "sin URL directa (curseforge no soportado aún)".into() });
                continue;
            }
        };

        let category = entry.file.split('/').next().unwrap_or("mods");
        let dest_dir = PathBuf::from(&instance_dir).join(category);
        tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;
        let dest_file = dest_dir.join(&meta.filename);

        if dest_file.exists() {
            results.push(SyncedMod { name: meta.name, filename: meta.filename, status: "ya presente".into() });
            continue;
        }

        match client.get(&download_url).send().await {
            Ok(r) if r.status().is_success() => {
                match r.bytes().await {
                    Ok(bytes) => {
                        if let Err(e) = tokio::fs::write(&dest_file, &bytes).await {
                            results.push(SyncedMod { name: meta.name, filename: meta.filename, status: format!("error guardando: {}", e) });
                        } else {
                            results.push(SyncedMod { name: meta.name, filename: meta.filename, status: "descargado".into() });
                        }
                    }
                    Err(e) => results.push(SyncedMod { name: meta.name, filename: meta.filename, status: format!("error descargando: {}", e) }),
                }
            }
            Ok(r) => results.push(SyncedMod { name: meta.name, filename: meta.filename, status: format!("HTTP {}", r.status()) }),
            Err(e) => results.push(SyncedMod { name: meta.name, filename: meta.filename, status: format!("error: {}", e) }),
        }
    }

    Ok(results)
}