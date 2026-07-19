use serde_json::Value;
use std::path::Path;

pub fn os_matches(rule_os: &Value) -> bool {
    match rule_os["name"].as_str() {
        Some("windows") => cfg!(target_os = "windows"),
        Some("osx") => cfg!(target_os = "macos"),
        Some("linux") => cfg!(target_os = "linux"),
        _ => true,
    }
}

pub fn library_allowed(lib: &Value) -> bool {
    let rules = match lib["rules"].as_array() {
        Some(r) => r,
        None => return true,
    };
    let mut allowed = false;
    for rule in rules {
        let action_allow = rule["action"].as_str() == Some("allow");
        let matches_os = rule.get("os").map(os_matches).unwrap_or(true);
        if matches_os {
            allowed = action_allow;
        }
    }
    allowed
}

pub fn maven_to_path(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return name.replace(':', "/");
    }
    let group = parts[0].replace('.', "/");
    let (artifact, version) = (parts[1], parts[2]);
    let classifier = parts.get(3).map(|c| format!("-{}", c)).unwrap_or_default();
    format!(
        "{}/{}/{}/{}-{}{}.jar",
        group, artifact, version, artifact, version, classifier
    )
}

pub fn build_classpath(instance_dir: &Path, version_json: &Value) -> Result<String, String> {
    let separator = if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    };
    let mut jars = Vec::new();

    if let Some(libs) = version_json["libraries"].as_array() {
        for lib in libs {
            if !library_allowed(lib) {
                continue;
            }
            if let Some(path) = lib["downloads"]["artifact"]["path"].as_str() {
                jars.push(
                    instance_dir
                        .join("libraries")
                        .join(path)
                        .to_string_lossy()
                        .to_string(),
                );
            } else if let Some(name) = lib["name"].as_str() {
                jars.push(
                    instance_dir
                        .join("libraries")
                        .join(maven_to_path(name))
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }

    let main_class = version_json["mainClass"].as_str().unwrap_or("");
    let is_modular_loader = main_class.starts_with("cpw.mods.");

    if !is_modular_loader {
        if let Some(vanilla_id) = version_json["_vanillaId"].as_str() {
            let client_jar = instance_dir
                .join("versions")
                .join(vanilla_id)
                .join(format!("{}.jar", vanilla_id));
            if client_jar.exists() {
                jars.push(client_jar.to_string_lossy().to_string());
            }
        }
    }

    Ok(jars.join(separator))
}

pub async fn ensure_libraries(
    instance_dir: &std::path::Path,
    version_json: &Value,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let libs = match version_json["libraries"].as_array() {
        Some(l) => l,
        None => return Ok(()),
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    for lib in libs {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Cancelado por el usuario".to_string());
        }
        if !library_allowed(lib) {
            continue;
        }

        let mut rel_path = String::new();
        let mut dl_url = String::new();

        if let Some(artifact) = lib.get("downloads").and_then(|d| d.get("artifact")) {
            if let (Some(p), Some(u)) = (artifact["path"].as_str(), artifact["url"].as_str()) {
                if !u.is_empty() {
                    rel_path = p.to_string();
                    dl_url = u.to_string();
                }
            }
        } else if let Some(name) = lib["name"].as_str() {
            rel_path = maven_to_path(name);
            if let Some(url) = lib.get("url").and_then(|u| u.as_str()) {
                dl_url = format!("{}{}", url, rel_path);
            }
        }

        if rel_path.is_empty() || dl_url.is_empty() {
            continue;
        }

        let dest = instance_dir.join("libraries").join(&rel_path);
        if dest.exists() {
            continue;
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }

        let resp = client
            .get(&dl_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            tokio::fs::write(&dest, &bytes)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
