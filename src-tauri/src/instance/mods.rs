use std::path::PathBuf;
use crate::instance::models::ModEntry;

pub fn mods_dir(instance_dir: &str) -> PathBuf {
    PathBuf::from(instance_dir).join("mods")
}

pub fn is_mod_file(filename: &str) -> bool {
    filename.ends_with(".jar") || filename.ends_with(".jar.disabled")
}

pub fn strip_mod_suffix(filename: &str) -> String {
    filename.trim_end_matches(".disabled").trim_end_matches(".jar").to_string()
}

#[tauri::command]
pub fn list_mods(instance_dir: String) -> Result<Vec<ModEntry>, String> {
    let dir = mods_dir(&instance_dir);
    if !dir.exists() { return Ok(vec![]); }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        if !entry.path().is_file() { continue; }
        let filename = entry.file_name().to_string_lossy().to_string();
        if !is_mod_file(&filename) { continue; }

        out.push(ModEntry {
            filename: filename.clone(),
            display_name: strip_mod_suffix(&filename),
            enabled: filename.ends_with(".jar"),
            size_kb: entry.metadata().map(|m| m.len() / 1024).unwrap_or(0),
        });
    }
    out.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn toggle_mod(instance_dir: String, filename: String, enable: bool) -> Result<String, String> {
    let dir = mods_dir(&instance_dir);
    let current = dir.join(&filename);
    if !current.exists() { return Err(format!("Archivo {} no existe", filename)); }

    let base = strip_mod_suffix(&filename);
    let new_file = if enable { format!("{}.jar", base) } else { format!("{}.jar.disabled", base) };
    let new_path = dir.join(&new_file);

    if current != new_path { std::fs::rename(&current, &new_path).unwrap(); }
    Ok(new_file)
}