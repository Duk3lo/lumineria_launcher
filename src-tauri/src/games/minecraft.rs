use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use crate::auth::AuthSession;
use futures_util::TryStreamExt;
use tauri::Emitter;

/// Opciones necesarias para lanzar una instancia.
/// serde(rename_all = "camelCase") para que del lado de JS se use
/// instanceDir, versionId, javaPath, ramMinMb, ramMaxMb, extraJavaArgs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub instance_dir: String,
    /// El nombre EXACTO de la carpeta dentro de versions/, ej "neoforge-21.0.167"
    /// o "1.20.1-forge-47.2.20". Para vanilla, es el mc_version tal cual.
    /// Agregá este campo a cada perfil en tu profiles.json.
    pub version_id: String,
    pub java_path: String,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub extra_java_args: String,
}

#[tauri::command]
pub async fn launch_minecraft(window: tauri::Window, options: LaunchOptions, auth: AuthSession) -> Result<(), String> {
    let instance_dir = PathBuf::from(&options.instance_dir);
    let version_json = load_merged_version(&instance_dir, &options.version_id).await?;

    ensure_assets(&window, &instance_dir, &version_json).await?;
    tokio::fs::create_dir_all(instance_dir.join("natives"))
        .await
        .map_err(|e| e.to_string())?;

    let classpath = build_classpath(&instance_dir, &version_json)?;
    let main_class = version_json["mainClass"]
        .as_str()
        .ok_or("mainClass no encontrado en el version.json fusionado")?
        .to_string();

    let mut vars = HashMap::new();
    vars.insert("auth_player_name".into(), auth.username.clone());
    vars.insert("version_name".into(), options.version_id.clone());
    vars.insert(
        "game_directory".into(),
        instance_dir.to_string_lossy().to_string(),
    );
    vars.insert(
        "assets_root".into(),
        instance_dir.join("assets").to_string_lossy().to_string(),
    );
    vars.insert(
        "assets_index_name".into(),
        version_json["assetIndex"]["id"]
            .as_str()
            .unwrap_or("legacy")
            .to_string(),
    );
    vars.insert("auth_uuid".into(), auth.uuid.clone());
    vars.insert("auth_access_token".into(), auth.access_token.clone());
    vars.insert("clientid".into(), "0".into());
    vars.insert("auth_xuid".into(), "0".into());
    vars.insert("user_type".into(), auth.user_type.clone());
    vars.insert("version_type".into(), "release".into());
    vars.insert(
        "natives_directory".into(),
        instance_dir.join("natives").to_string_lossy().to_string(),
    );
    vars.insert("launcher_name".into(), "LumineriaLauncher".into());
    vars.insert("launcher_version".into(), "1.0.0".into());
    vars.insert("classpath".into(), classpath);

    let jvm_args = extract_argument_list(&version_json["arguments"]["jvm"], &vars);
    let game_args = extract_argument_list(&version_json["arguments"]["game"], &vars);

    let mut command = Command::new(&options.java_path);
    command.current_dir(&instance_dir);
    command.arg(format!("-Xms{}M", options.ram_min_mb));
    command.arg(format!("-Xmx{}M", options.ram_max_mb));

    for a in options.extra_java_args.split_whitespace() {
        command.arg(a);
    }
    for a in jvm_args {
        command.arg(a);
    }
    command.arg(&main_class);
    for a in game_args {
        command.arg(a);
    }

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("No se pudo lanzar Java: {}", e))?;

    // No bloqueamos el launcher esperando a que cierres el juego.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    Ok(())
}

// ---------- Carga y fusión de version.json (soporta inheritsFrom) ----------

async fn load_version_json(instance_dir: &Path, version_id: &str) -> Result<Value, String> {
    let path = instance_dir
        .join("versions")
        .join(version_id)
        .join(format!("{}.json", version_id));
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("No se encontró {}: {}", path.display(), e))?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn load_merged_version<'a>(
    instance_dir: &'a Path,
    version_id: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + 'a>> {
    Box::pin(async move {
        let mut current = load_version_json(instance_dir, version_id).await?;

        if let Some(parent_id) = current["inheritsFrom"].as_str().map(|s| s.to_string()) {
            let parent = load_merged_version(instance_dir, &parent_id).await?;
            let vanilla_id = parent["_vanillaId"]
                .as_str()
                .unwrap_or(&parent_id)
                .to_string();
            current = merge_versions(parent, current);
            current["_vanillaId"] = Value::String(vanilla_id);
        } else {
            current["_vanillaId"] = Value::String(version_id.to_string());
        }

        Ok(current)
    })
}

fn merge_versions(parent: Value, mut child: Value) -> Value {
    let mut libs = parent["libraries"].as_array().cloned().unwrap_or_default();
    if let Some(child_libs) = child["libraries"].as_array() {
        libs.extend(child_libs.clone());
    }
    child["libraries"] = Value::Array(libs);

    for key in ["jvm", "game"] {
        let mut merged = parent["arguments"][key]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if let Some(child_args) = child["arguments"][key].as_array() {
            merged.extend(child_args.clone());
        }
        child["arguments"][key] = Value::Array(merged);
    }

    for key in ["assetIndex", "downloads", "assets"] {
        if child.get(key).map(Value::is_null).unwrap_or(true) {
            child[key] = parent[key].clone();
        }
    }
    if child.get("mainClass").map(Value::is_null).unwrap_or(true) {
        child["mainClass"] = parent["mainClass"].clone();
    }

    child
}

// ---------- Classpath ----------

fn os_matches(rule_os: &Value) -> bool {
    match rule_os["name"].as_str() {
        Some("windows") => cfg!(target_os = "windows"),
        Some("osx") => cfg!(target_os = "macos"),
        Some("linux") => cfg!(target_os = "linux"),
        _ => true,
    }
}

fn library_allowed(lib: &Value) -> bool {
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

fn maven_to_path(name: &str) -> String {
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

fn build_classpath(instance_dir: &Path, version_json: &Value) -> Result<String, String> {
    let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
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

    // El .jar del cliente vanilla vive en versions/<vanillaId>/<vanillaId>.jar
    if let Some(vanilla_id) = version_json["_vanillaId"].as_str() {
        let client_jar = instance_dir
            .join("versions")
            .join(vanilla_id)
            .join(format!("{}.jar", vanilla_id));
        if client_jar.exists() {
            jars.push(client_jar.to_string_lossy().to_string());
        }
    }

    Ok(jars.join(separator))
}

// ---------- Argumentos (jvm/game) con sustitución de variables ----------

fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("${{{}}}", k), v);
    }
    result
}

fn extract_argument_list(value: &Value, vars: &HashMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    let arr = match value.as_array() {
        Some(a) => a,
        None => return out,
    };

    for item in arr {
        match item {
            Value::String(s) => out.push(substitute(s, vars)),
            Value::Object(_) => {
                // Argumentos condicionales (resolución custom, demo, etc).
                // Los omitimos salvo que no tengan "features" y estén permitidos.
                let rules_ok = item["rules"]
                    .as_array()
                    .map(|rules| {
                        rules
                            .iter()
                            .all(|r| r["action"].as_str() == Some("allow") && r.get("features").is_none())
                    })
                    .unwrap_or(false);
                if rules_ok {
                    if let Some(val) = item["value"].as_str() {
                        out.push(substitute(val, vars));
                    } else if let Some(vals) = item["value"].as_array() {
                        for v in vals {
                            if let Some(s) = v.as_str() {
                                out.push(substitute(s, vars));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

// ---------- Descarga de assets (index + objects) ----------

async fn ensure_assets(
    window: &tauri::Window,
    instance_dir: &Path,
    version_json: &Value,
) -> Result<(), String> {
    let asset_id = version_json["assetIndex"]["id"].as_str().unwrap_or("legacy");
    let asset_url = version_json["assetIndex"]["url"].as_str();

    let indexes_dir = instance_dir.join("assets").join("indexes");
    tokio::fs::create_dir_all(&indexes_dir).await.map_err(|e| e.to_string())?;
    let index_path = indexes_dir.join(format!("{}.json", asset_id));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30)) // evita que una request colgada trabe todo
        .build()
        .map_err(|e| e.to_string())?;

    let index_json: Value = if index_path.exists() {
        let raw = tokio::fs::read_to_string(&index_path).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())?
    } else {
        let url = asset_url.ok_or("No hay assetIndex.url en el version.json")?;
        let raw = client.get(url).send().await.map_err(|e| e.to_string())?
            .text().await.map_err(|e| e.to_string())?;
        tokio::fs::write(&index_path, &raw).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())?
    };

    let objects_dir = instance_dir.join("assets").join("objects");
    let objects = index_json["objects"].as_object().cloned().unwrap_or_default();

    // Filtramos primero los que faltan, para saber el total real y poder
    // reportar progreso "X de Y" en vez de contar todos los objetos del index.
    let mut pending = Vec::new();
    for meta in objects.values() {
        if let Some(hash) = meta["hash"].as_str() {
            let prefix = &hash[0..2];
            let dest = objects_dir.join(prefix).join(hash);
            if !dest.exists() {
                pending.push(hash.to_string());
            }
        }
    }

    let total = pending.len();
    if total == 0 {
        return Ok(());
    }

    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let objects_dir = std::sync::Arc::new(objects_dir);

    futures_util::stream::iter(pending)
        .map(|hash| {
            let client = client.clone();
            let objects_dir = objects_dir.clone();
            let done = done.clone();
            let window = window.clone();
            async move {
                let prefix = hash[0..2].to_string();
                let dest_dir = objects_dir.join(&prefix);
                let dest_file = dest_dir.join(&hash);
                tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| e.to_string())?;

                let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);
                let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
                let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                tokio::fs::write(&dest_file, &bytes).await.map_err(|e| e.to_string())?;

                let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                let _ = window.emit("assets-progress", serde_json::json!({ "done": n, "total": total }));
                Ok::<(), String>(())
            }
        })
        .buffer_unordered(24) // 24 descargas simultáneas
        .try_for_each(|_| async { Ok(()) })
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn ensure_vanilla_version(instance_dir: String, mc_version: String) -> Result<(), String> {
    let instance_dir = PathBuf::from(&instance_dir);
    let version_dir = instance_dir.join("versions").join(&mc_version);
    let json_path = version_dir.join(format!("{}.json", mc_version));
    let jar_path = version_dir.join(format!("{}.jar", mc_version));

    tokio::fs::create_dir_all(&version_dir).await.map_err(|e| e.to_string())?;

    let version_json: Value = if json_path.exists() {
        let raw = tokio::fs::read_to_string(&json_path).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())?
    } else {
        let manifest: Value = reqwest::get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
            .await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;

        let entry_url = manifest["versions"].as_array()
            .and_then(|arr| arr.iter().find(|v| v["id"].as_str() == Some(mc_version.as_str())))
            .and_then(|v| v["url"].as_str())
            .ok_or_else(|| format!("No se encontró la versión {} en el manifest de Mojang", mc_version))?
            .to_string();

        let raw = reqwest::get(&entry_url).await.map_err(|e| e.to_string())?
            .text().await.map_err(|e| e.to_string())?;
        tokio::fs::write(&json_path, &raw).await.map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())?
    };

    if !jar_path.exists() {
        if let Some(client_url) = version_json["downloads"]["client"]["url"].as_str() {
            let bytes = reqwest::get(client_url).await.map_err(|e| e.to_string())?
                .bytes().await.map_err(|e| e.to_string())?;
            tokio::fs::write(&jar_path, &bytes).await.map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}