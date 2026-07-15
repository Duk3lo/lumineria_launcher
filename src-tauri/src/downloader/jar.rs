use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use std::process::Stdio;
use std::path::PathBuf;

#[tauri::command]
pub async fn execute_jar(window: tauri::Window, java_path: String, jar_path: String, args: Vec<String>, work_dir: String) -> Result<String, String> {
    let mut command = Command::new(&java_path);
    command.current_dir(&work_dir).arg("-jar").arg(&jar_path);
    for arg in &args { command.arg(arg); }
    
    // Configuramos Pipes para lectura
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("Fallo al ejecutar Java: {}", e))?;
    
    // Tomamos los lectores de salida
    let stdout_reader = child.stdout.take().unwrap();
    let stderr_reader = child.stderr.take().unwrap();

    // Tarea para STDOUT: Procesar y enviar al frontend inmediatamente
    let window_out = window.clone();
    let out_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout_reader).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = window_out.emit("process-log", serde_json::json!({ "stream": "stdout", "line": &line }));
        }
    });

    // Tarea para STDERR: Lo mismo para errores
    let window_err = window.clone();
    let err_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr_reader).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = window_err.emit("process-log", serde_json::json!({ "stream": "stderr", "line": &line }));
        }
    });

    // Esperar a que el proceso termine
    let status = child.wait().await.unwrap();
    
    // Forzamos el fin de las tareas de lectura para liberar recursos
    out_task.abort();
    err_task.abort();

    if status.success() {
        Ok("Instalación completada exitosamente".into())
    } else {
        Err("El instalador devolvió un código de error. Revisa la consola del launcher.".into())
    }
}

// NUEVO COMANDO (Para no instalar forge a cada rato)
#[tauri::command]
pub async fn check_version_installed(instance_dir: String, version_id: String) -> Result<bool, String> {
    let json_path = PathBuf::from(instance_dir).join("versions").join(&version_id).join(format!("{}.json", version_id));
    Ok(json_path.exists())
}