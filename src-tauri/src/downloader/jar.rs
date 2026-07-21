use std::path::PathBuf;
use std::process::Stdio;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::net::HideConsoleExt;

#[tauri::command]
pub async fn execute_jar(
    window: tauri::Window,
    java_path: String,
    jar_path: String,
    args: Vec<String>,
    work_dir: String,
) -> Result<String, String> {
    let mut command = Command::new(&java_path);
    command
        .current_dir(&work_dir)
        .arg("-jar")
        .arg(&jar_path)
        .kill_on_drop(true);
    for arg in &args {
        command.arg(arg);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.hide_console();

    let mut child = command
        .spawn()
        .map_err(|e| format!("Fallo al ejecutar Java: {}", e))?;
    let stdout_reader = child.stdout.take().ok_or("No se pudo capturar stdout del proceso")?;
    let stderr_reader = child.stderr.take().ok_or("No se pudo capturar stderr del proceso")?;

    let window_out = window.clone();
    let out_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout_reader).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = window_out.emit(
                "process-log",
                serde_json::json!({ "stream": "stdout", "line": &line }),
            );
        }
    });
    let window_err = window.clone();
    let err_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr_reader).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = window_err.emit(
                "process-log",
                serde_json::json!({ "stream": "stderr", "line": &line }),
            );
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Error esperando al proceso de Java: {}", e))?;
    out_task.abort();
    err_task.abort();

    if status.success() {
        Ok("Instalación completada exitosamente".into())
    } else {
        Err("El instalador devolvió un código de error. Revisa la consola del launcher.".into())
    }
}

#[tauri::command]
pub async fn check_version_installed(instance_dir: String, version_id: String) -> Result<bool, String> {
    let json_path = PathBuf::from(instance_dir)
        .join("versions")
        .join(&version_id)
        .join(format!("{}.json", version_id));
    Ok(json_path.exists())
}
