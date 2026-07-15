use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Un mod dentro de la carpeta `mods/` de una instancia.
/// `enabled = true`  -> el archivo termina en ".jar"
/// `enabled = false` -> el archivo termina en ".jar.disabled" (convención estándar,
/// la misma que usan CurseForge / MultiMC / Prism para "desactivar" un mod sin borrarlo).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub filename: String,
    pub display_name: String,
    pub enabled: bool,
    pub size_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStatus {
    pub installed: bool,
    pub mods_count: u32,
}

fn mods_dir(instance_dir: &str) -> PathBuf {
    PathBuf::from(instance_dir).join("mods")
}

fn is_mod_file(filename: &str) -> bool {
    filename.ends_with(".jar") || filename.ends_with(".jar.disabled")
}

fn strip_mod_suffix(filename: &str) -> String {
    filename
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar")
        .to_string()
}

/// Chequeo rápido y liviano para decidir si el botón de una tarjeta dice
/// "Jugar" o "Instalar": si existe versions/<algo>/ ya se instaló al menos una vez.
#[tauri::command]
pub fn get_instance_status(instance_dir: String) -> InstanceStatus {
    let base = PathBuf::from(&instance_dir);
    let versions_dir = base.join("versions");

    let installed = versions_dir
        .read_dir()
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);

    let mods_count = mods_dir(&instance_dir)
        .read_dir()
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| is_mod_file(&e.file_name().to_string_lossy()))
                .count() as u32
        })
        .unwrap_or(0);

    InstanceStatus {
        installed,
        mods_count,
    }
}

/// Lista los mods instalados en una instancia (carpeta `mods/`).
#[tauri::command]
pub fn list_mods(instance_dir: String) -> Result<Vec<ModEntry>, String> {
    let dir = mods_dir(&instance_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        if !is_mod_file(&filename) {
            continue;
        }

        let enabled = filename.ends_with(".jar");
        let display_name = strip_mod_suffix(&filename);
        let size_kb = entry.metadata().map(|m| m.len() / 1024).unwrap_or(0);

        out.push(ModEntry {
            filename,
            display_name,
            enabled,
            size_kb,
        });
    }

    out.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    Ok(out)
}

/// Activa o desactiva un mod renombrando el archivo (agrega/quita ".disabled").
/// Devuelve el nuevo nombre de archivo para que el frontend actualice su estado.
#[tauri::command]
pub fn toggle_mod(instance_dir: String, filename: String, enable: bool) -> Result<String, String> {
    let dir = mods_dir(&instance_dir);
    let current_path = dir.join(&filename);

    if !current_path.exists() {
        return Err(format!("No se encontró el archivo {}", filename));
    }

    let base_name = strip_mod_suffix(&filename);
    let new_filename = if enable {
        format!("{}.jar", base_name)
    } else {
        format!("{}.jar.disabled", base_name)
    };
    let new_path = dir.join(&new_filename);

    if current_path != new_path {
        std::fs::rename(&current_path, &new_path).map_err(|e| e.to_string())?;
    }

    Ok(new_filename)
}

#[tauri::command]
pub async fn reset_instance_libraries(instance_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(&instance_dir);
    for sub in ["libraries", "versions"] {
        let path = dir.join(sub);
        if path.exists() {
            tokio::fs::remove_dir_all(&path)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}