use std::path::PathBuf;
use std::time::SystemTime;

/// Después de correr el instalador de Forge/NeoForge, el id de versión que
/// realmente queda en disco no siempre coincide con el que armamos a mano
/// (cambia de formato según la versión del instalador). Escanea `versions/`
/// y devuelve la carpeta recién creada por el loader, descartando la
/// vanilla y la de Fabric (que sí tienen nombre predecible y se manejan
/// aparte).
#[tauri::command]
pub async fn detect_installed_loader_version(
    instance_dir: String,
    mc_version: String,
) -> Result<Option<String>, String> {
    let versions_dir = PathBuf::from(&instance_dir).join("versions");
    if !versions_dir.exists() {
        return Ok(None);
    }

    let mut entries = tokio::fs::read_dir(&versions_dir)
        .await
        .map_err(|e| e.to_string())?;

    let mut best: Option<(String, SystemTime)> = None;

    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();

        // la vanilla pura y Fabric ya tienen nombre conocido, no son esto
        if id == mc_version || id.starts_with("fabric-loader-") {
            continue;
        }

        let json_path = path.join(format!("{}.json", id));
        if !json_path.exists() {
            continue; // carpeta incompleta, no cuenta
        }

        let modified = entry
            .metadata()
            .await
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let is_newer = match &best {
            Some((_, best_time)) => modified > *best_time,
            None => true,
        };
        if is_newer {
            best = Some((id, modified));
        }
    }

    Ok(best.map(|(id, _)| id))
}