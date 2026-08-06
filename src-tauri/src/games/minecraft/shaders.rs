use std::path::{Path, PathBuf};

use crate::gpu;

const CANDIDATE_FILES: [&str; 2] = ["iris.properties", "oculus.properties"];
const ENABLE_KEY: &str = "enableShaders";


pub async fn apply_shader_gpu_policy(instance_dir: &Path, force_enabled: bool) -> Result<(), String> {
    if force_enabled || gpu::has_good_gpu().await {
        return Ok(());
    }

    let config_dir = instance_dir.join("config");
    for filename in CANDIDATE_FILES {
        let path = config_dir.join(filename);
        if path.exists() {
            disable_shaders_in_file(&path).await?;
        }
    }
    Ok(())
}

async fn disable_shaders_in_file(path: &PathBuf) -> Result<(), String> {
    let content = tokio::fs::read_to_string(path).await.map_err(|e| e.to_string())?;

    let mut found = false;
    let mut lines: Vec<String> = content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(ENABLE_KEY)
                && trimmed[ENABLE_KEY.len()..].trim_start().starts_with('=')
            {
                found = true;
                format!("{}=false", ENABLE_KEY)
            } else {
                line.to_string()
            }
        })
        .collect();

    if !found {
        lines.push(format!("{}=false", ENABLE_KEY));
    }

    let mut new_content = lines.join("\n");
    new_content.push('\n');

    tokio::fs::write(path, new_content).await.map_err(|e| e.to_string())
}


#[tauri::command]
pub async fn apply_shader_gpu_policy_command(
    instance_dir: String,
    force_enabled: bool,
) -> Result<(), String> {
    apply_shader_gpu_policy(Path::new(&instance_dir), force_enabled).await
}