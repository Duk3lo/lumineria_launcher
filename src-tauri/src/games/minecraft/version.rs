use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub async fn load_version_json(instance_dir: &Path, version_id: &str) -> Result<Value, String> {
    let path = instance_dir
        .join("versions")
        .join(version_id)
        .join(format!("{}.json", version_id));
    let raw = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn load_merged_version<'a>(
    instance_dir: &'a Path,
    version_id: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + 'a>> {
    Box::pin(async move {
        let mut current = load_version_json(instance_dir, version_id).await?;
        if let Some(parent_id) = current["inheritsFrom"].as_str().map(|s| s.to_string()) {
            let parent = load_merged_version(instance_dir, &parent_id).await?;
            let vanilla_id = parent["_vanillaId"].as_str().unwrap_or(&parent_id).to_string();
            current = merge_versions(parent, current);
            current["_vanillaId"] = Value::String(vanilla_id);
        } else {
            current["_vanillaId"] = Value::String(version_id.to_string());
        }
        Ok(current)
    })
}

pub fn merge_versions(parent: Value, mut child: Value) -> Value {
    let mut libs = parent["libraries"].as_array().cloned().unwrap_or_default();
    if let Some(child_libs) = child["libraries"].as_array() {
        libs.extend(child_libs.clone());
    }
    child["libraries"] = Value::Array(dedupe_libraries(&libs));

    for key in ["jvm", "game"] {
        let mut merged = parent["arguments"][key].as_array().cloned().unwrap_or_default();
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

pub fn library_key(lib: &Value) -> String {
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

pub fn dedupe_libraries(libs: &[Value]) -> Vec<Value> {
    let mut order = Vec::new();
    let mut map = HashMap::new();

    for lib in libs {
        let key = library_key(lib);
        let has_downloads = lib["downloads"]["artifact"]["path"].as_str().is_some();
        match map.get(&key) {
            None => {
                order.push(key.clone());
                map.insert(key, lib.clone());
            }
            Some(existing) => {
                let existing_has_downloads = existing["downloads"]["artifact"]["path"].as_str().is_some();
                if has_downloads || !existing_has_downloads {
                    map.insert(key, lib.clone());
                }
            }
        }
    }
    order.into_iter().filter_map(|k| map.remove(&k)).collect()
}
