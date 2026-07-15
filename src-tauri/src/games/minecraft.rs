use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::auth::AuthSession;
use futures_util::TryStreamExt;
use tauri::Emitter;

/// Opciones necesarias para lanzar una instancia.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub instance_dir: String,
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

    // --- FIX: asegurar que TODAS las librerías con descarga conocida existan ---
    // Esto es crítico para instancias Vanilla (que antes nunca bajaban libraries/)
    // y sirve de red de seguridad si el instalador de NeoForge/Forge quedó incompleto.
    ensure_libraries(&instance_dir, &version_json).await?;

    ensure_assets(&window, &instance_dir, &version_json).await?;
    tokio::fs::create_dir_all(instance_dir.join("natives"))
        .await
        .map_err(|e| e.to_string())?;

    let classpath = build_classpath(&instance_dir, &version_json, &options.version_id)?;
    let main_class = version_json["mainClass"]
        .as_str()
        .ok_or("mainClass no encontrado en el version.json fusionado")?
        .to_string();

    // --- FIX: separador de classpath según el SO, usado para armar library_directory ---
    let classpath_separator = if cfg!(target_os = "windows") { ";" } else { ":" };

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

    // --- FIX PRINCIPAL: placeholders que usa el module-path (-p) de NeoForge/Forge
    // modernos. Sin esto, "${library_directory}" queda literal en el argumento -p
    // y el BootstrapLauncher no puede resolver módulos como datafixers/fastutil/guava.
    vars.insert("classpath_separator".into(), classpath_separator.to_string());
    vars.insert(
        "library_directory".into(),
        instance_dir.join("libraries").to_string_lossy().to_string(),
    );

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

    if let Some(stdout) = child.stdout.take() {
        let window_out = window.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        println!("[MC stdout] {line}");
                        let _ = window_out.emit("game-log", &line);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("Error leyendo stdout de Minecraft: {e}");
                        break;
                    }
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let window_err = window.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        eprintln!("[MC stderr] {line}");
                        let _ = window_err.emit("game-log", &line);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("Error leyendo stderr de Minecraft: {e}");
                        break;
                    }
                }
            }
        });
    }

    let window_exit = window.clone();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => {
                if !status.success() {
                    let msg = format!("Minecraft terminó con código de error: {status}");
                    eprintln!("{msg}");
                    let _ = window_exit.emit("game-exit-error", &msg);
                } else {
                    println!("Minecraft cerró correctamente.");
                }
            }
            Err(e) => {
                let msg = format!("Error esperando el proceso de Minecraft: {e}");
                eprintln!("{msg}");
                let _ = window_exit.emit("game-exit-error", &msg);
            }
        }
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
    child["libraries"] = Value::Array(dedupe_libraries(&libs));

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

fn build_classpath(
    instance_dir: &Path,
    version_json: &Value,
    requested_version_id: &str,
) -> Result<String, String> {
    let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
    let mut jars = Vec::new();

    if let Some(libs) = version_json["libraries"].as_array() {
        for lib in libs {
            if !library_allowed(lib) {
                continue;
            }
            if let Some(path) = lib["downloads"]["artifact"]["path"].as_str() {
                jars.push(instance_dir.join("libraries").join(path).to_string_lossy().to_string());
            } else if let Some(name) = lib["name"].as_str() {
                jars.push(instance_dir.join("libraries").join(maven_to_path(name)).to_string_lossy().to_string());
            }
        }
    }

    // Solo agregamos el jar vanilla "crudo" si NO hay mod loader, es decir,
    // si lo que se pidió lanzar es directamente la versión vanilla.
    // Con NeoForge/Forge modernos el jar parcheado (con patches ya aplicados)
    // viene declarado como library y sustituye por completo al vanilla;
    // agregar los dos duplica las clases del juego bajo dos módulos y
    // rompe la resolución de módulos de Java.
    if let Some(vanilla_id) = version_json["_vanillaId"].as_str() {
        if vanilla_id == requested_version_id {
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

// ---------- FIX: descarga de librerías (libraries/) ----------
// Antes NADA descargaba las libraries listadas en el version.json: para
// instancias sin loader (Vanilla) esto significaba lanzar el juego sin
// datafixers/fastutil/guava/etc., causando "Can't Find Class" en cadena.
// Solo bajamos las que tienen downloads.artifact.url/path explícito (así
// funciona el manifest de Mojang). Las librerías propias de NeoForge/Forge
// que no traen esa info las sigue poniendo el instalador del loader.
async fn ensure_libraries(instance_dir: &Path, version_json: &Value) -> Result<(), String> {
    let libs = match version_json["libraries"].as_array() {
        Some(l) => l,
        None => return Ok(()),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    for lib in libs {
        if !library_allowed(lib) {
            continue;
        }

        let artifact = &lib["downloads"]["artifact"];
        let (rel_path, url) = match (artifact["path"].as_str(), artifact["url"].as_str()) {
            (Some(p), Some(u)) if !u.is_empty() => (p.to_string(), u.to_string()),
            _ => continue,
        };

        let dest = instance_dir.join("libraries").join(&rel_path);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
        }

        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!(
                "Error {} descargando la librería {}",
                resp.status(),
                url
            ));
        }
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        tokio::fs::write(&dest, &bytes).await.map_err(|e| e.to_string())?;
    }

    Ok(())
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
                // FIX: antes no se comprobaba el campo "os" de cada regla, así que
                // argumentos exclusivos de una plataforma (ej. -XstartOnFirstThread,
                // solo válido en macOS) se agregaban en CUALQUIER sistema operativo,
                // rompiendo el arranque de la JVM ("Unrecognized option").
                let rules_ok = item["rules"]
                    .as_array()
                    .map(|rules| {
                        let mut allowed = false;
                        for rule in rules {
                            let action_allow = rule["action"].as_str() == Some("allow");
                            let matches_os = rule.get("os").map(os_matches).unwrap_or(true);
                            let has_features = rule.get("features").is_some();
                            if matches_os && !has_features {
                                allowed = action_allow;
                            }
                        }
                        allowed
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
        .timeout(std::time::Duration::from_secs(30))
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
        .buffer_unordered(24)
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

/// Clave única de una librería para deduplicar: "group:artifact[:classifier]".
/// Ignoramos la versión a propósito: si vanilla y el loader (NeoForge/Forge)
/// declaran el mismo artefacto con distinta versión, nos quedamos con uno solo.
fn library_key(lib: &Value) -> String {
    if let Some(name) = lib["name"].as_str() {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() >= 3 {
            let mut key = format!("{}:{}", parts[0], parts[1]);
            if let Some(classifier) = parts.get(3) {
                key.push(':');
                key.push_str(classifier);
            }
            return key;
        }
        return name.to_string();
    }
    lib["downloads"]["artifact"]["path"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// FIX: elimina duplicados exactos (mismo artefacto) que aparecen cuando se
/// fusiona la lista de libraries del padre (vanilla) con la del hijo
/// (NeoForge/Forge). Sin esto, BootstrapLauncher explota con
/// "Duplicate key ..." al construir el UnionFileSystem del module layer.
/// Se queda con la ÚLTIMA aparición (la del loader pisa a la de vanilla)
/// pero conserva el orden de la primera aparición.
fn dedupe_libraries(libs: &[Value]) -> Vec<Value> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Value> = HashMap::new();

    for lib in libs {
        let key = library_key(lib);
        let has_downloads = lib["downloads"]["artifact"]["path"].as_str().is_some();

        match map.get(&key) {
            None => {
                order.push(key.clone());
                map.insert(key, lib.clone());
            }
            Some(existing) => {
                let existing_has_downloads =
                    existing["downloads"]["artifact"]["path"].as_str().is_some();
                // FIX: nunca pisamos una entrada CON info de descarga por una que
                // no la tiene. Si ambas la tienen (o ninguna), gana la más nueva
                // (la del loader), que es la que normalmente debe prevalecer.
                if has_downloads || !existing_has_downloads {
                    map.insert(key, lib.clone());
                }
            }
        }
    }

    order.into_iter().filter_map(|k| map.remove(&k)).collect()
}