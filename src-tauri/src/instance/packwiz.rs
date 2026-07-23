use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
    #[serde(rename = "hash-format", default)]
    hash_format: Option<String>,
    #[serde(default)]
    hash: Option<String>,
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

fn hash_bytes(bytes: &[u8], format: &str) -> Option<String> {
    match format.to_lowercase().as_str() {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(bytes);
            Some(hex::encode(hasher.finalize()))
        }
        "sha1" => {
            let mut hasher = Sha1::new();
            hasher.update(bytes);
            Some(hex::encode(hasher.finalize()))
        }
        _ => None,
    }
}

enum UpdateReason {
    Missing,
    HashMismatch,
    SizeMismatch,
    UpToDate,
    Unverifiable,
}

async fn check_update_needed(
    dest_file: &Path,
    expected_hash: Option<&str>,
    hash_format: Option<&str>,
    download_url: &str,
    client: &reqwest::Client,
) -> UpdateReason {
    if !dest_file.exists() {
        return UpdateReason::Missing;
    }

    let local_bytes = match tokio::fs::read(dest_file).await {
        Ok(b) => b,
        Err(_) => return UpdateReason::Missing,
    };

    if let (Some(expected), Some(format)) = (expected_hash, hash_format) {
        if let Some(local_hash) = hash_bytes(&local_bytes, format) {
            return if local_hash.eq_ignore_ascii_case(expected) {
                UpdateReason::UpToDate
            } else {
                UpdateReason::HashMismatch
            };
        }
    }

    match client.head(download_url).send().await {
        Ok(resp) => match resp.content_length() {
            Some(remote_len) => {
                if remote_len as usize != local_bytes.len() {
                    UpdateReason::SizeMismatch
                } else {
                    UpdateReason::UpToDate
                }
            }
            None => UpdateReason::Unverifiable,
        },
        Err(_) => UpdateReason::Unverifiable,
    }
}

#[tauri::command]
pub async fn sync_packwiz_modpack(pack_url: String, instance_dir: String) -> Result<Vec<SyncedMod>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let base = base_url(&pack_url);

    let pack_raw = client
        .get(&pack_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let pack: PackToml = toml::from_str(&pack_raw).map_err(|e| format!("pack.toml inválido: {}", e))?;

    let index_url = resolve_url(&base, &pack.index.file);
    let index_raw = client
        .get(&index_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let index: IndexToml = toml::from_str(&index_raw).map_err(|e| format!("index.toml inválido: {}", e))?;

    let mut results = Vec::new();

    for entry in index.files.iter().filter(|f| f.metafile) {
        let meta_url = resolve_url(&base, &entry.file);

        let meta_raw = match client.get(&meta_url).send().await {
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(e) => {
                results.push(SyncedMod {
                    name: entry.file.clone(),
                    filename: entry.file.clone(),
                    status: format!("error: {}", e),
                });
                continue;
            }
        };

        let meta: ModMetaToml = match toml::from_str(&meta_raw) {
            Ok(m) => m,
            Err(e) => {
                results.push(SyncedMod {
                    name: entry.file.clone(),
                    filename: entry.file.clone(),
                    status: format!("error parseando: {}", e),
                });
                continue;
            }
        };

        if meta.side.as_deref() == Some("server") {
            results.push(SyncedMod {
                name: meta.name,
                filename: meta.filename,
                status: "omitido (server-only)".into(),
            });
            continue;
        }

        let download_url = match &meta.download.url {
            Some(u) => u.clone(),
            None => {
                results.push(SyncedMod {
                    name: meta.name,
                    filename: meta.filename,
                    status: "sin URL directa (curseforge no soportado aún)".into(),
                });
                continue;
            }
        };

        let category = entry.file.split('/').next().unwrap_or("mods");
        let dest_dir = PathBuf::from(&instance_dir).join(category);
        tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;
        let dest_file = dest_dir.join(&meta.filename);

        let reason = check_update_needed(
            &dest_file,
            meta.download.hash.as_deref(),
            meta.download.hash_format.as_deref(),
            &download_url,
            &client,
        )
        .await;

        let (should_download, skip_status): (bool, Option<&str>) = match reason {
            UpdateReason::Missing | UpdateReason::HashMismatch | UpdateReason::SizeMismatch => (true, None),
            UpdateReason::UpToDate => (false, Some("ya presente (verificado)")),
            UpdateReason::Unverifiable => (false, Some("ya presente (sin verificar)")),
        };

        if !should_download {
            results.push(SyncedMod {
                name: meta.name,
                filename: meta.filename,
                status: skip_status.unwrap().into(),
            });
            continue;
        }

        let was_update = dest_file.exists();

        match client.get(&download_url).send().await {
            Ok(r) if r.status().is_success() => match r.bytes().await {
                Ok(bytes) => {
                    if let Err(e) = tokio::fs::write(&dest_file, &bytes).await {
                        results.push(SyncedMod {
                            name: meta.name,
                            filename: meta.filename,
                            status: format!("error guardando: {}", e),
                        });
                    } else {
                        results.push(SyncedMod {
                            name: meta.name,
                            filename: meta.filename,
                            status: if was_update { "actualizado".into() } else { "descargado".into() },
                        });
                    }
                }
                Err(e) => results.push(SyncedMod {
                    name: meta.name,
                    filename: meta.filename,
                    status: format!("error descargando: {}", e),
                }),
            },
            Ok(r) => results.push(SyncedMod {
                name: meta.name,
                filename: meta.filename,
                status: format!("HTTP {}", r.status()),
            }),
            Err(e) => results.push(SyncedMod {
                name: meta.name,
                filename: meta.filename,
                status: format!("error: {}", e),
            }),
        }
    }

    Ok(results)
}