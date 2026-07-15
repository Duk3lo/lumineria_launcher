use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

use futures_util::StreamExt;
use tauri::Emitter;

use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};

// --- 1. COMANDO PARA DESCARGAR ARCHIVOS GENÉRICOS ---

#[tauri::command]
pub async fn download_generic_file(url: String, dest_path: String) -> Result<String, String> {
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "El servidor respondió {} al descargar {}",
            response.status(),
            url
        ));
    }
    let mut file = tokio::fs::File::create(&dest_path).await.map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }

    Ok(dest_path)
}

// --- 2. COMANDO PARA EJECUTAR .JAR CON LOGS EN VIVO ---

#[tauri::command]
pub async fn execute_jar(
    window: tauri::Window,
    java_path: String,
    jar_path: String,
    args: Vec<String>,
    work_dir: String,
) -> Result<String, String> {
    let mut command = Command::new(&java_path);
    command.current_dir(&work_dir);
    command.arg("-jar").arg(&jar_path);
    for arg in &args {
        command.arg(arg);
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("No se pudo lanzar {}: {}", java_path, e))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_buf = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let stderr_buf = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let out_task = {
        let window = window.clone();
        let buf = stdout_buf.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window.emit("process-log", serde_json::json!({ "stream": "stdout", "line": line }));
                buf.lock().await.push(line);
            }
        })
    };

    let err_task = {
        let window = window.clone();
        let buf = stderr_buf.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window.emit("process-log", serde_json::json!({ "stream": "stderr", "line": line }));
                buf.lock().await.push(line);
            }
        })
    };

    let status = child.wait().await.map_err(|e| e.to_string())?;
    let _ = out_task.await;
    let _ = err_task.await;

    let stdout = stdout_buf.lock().await.join("\n");
    let stderr = stderr_buf.lock().await.join("\n");

    if status.success() {
        Ok(stdout)
    } else {
        // Muchos instaladores (Forge/NeoForge incluidos) imprimen los errores
        // por stdout, no por stderr, así que devolvemos los dos. También
        // agregamos el código de salida y, si existe, la cola del
        // installer.log que estos instaladores dejan al lado del jar.
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "desconocido (killed por señal)".to_string());

        let mut msg = format!(
            "Error ejecutando jar '{}' (código de salida: {}).\n--- stdout ---\n{}\n--- stderr ---\n{}",
            jar_path,
            code,
            if stdout.trim().is_empty() { "(vacío)" } else { &stdout },
            if stderr.trim().is_empty() { "(vacío)" } else { &stderr },
        );

        if let Some(log_tail) = read_installer_log_tail(&work_dir) {
            msg.push_str(&format!("\n--- installer.log (final) ---\n{}", log_tail));
        }

        Err(msg)
    }
}

/// Forge/NeoForge escriben "installer.log" en la carpeta de trabajo con el
/// stack trace real cuando la instalación falla. Devolvemos solo las
/// últimas líneas para no inundar el mensaje de error en la UI.
fn read_installer_log_tail(work_dir: &str) -> Option<String> {
    let log_path = PathBuf::from(work_dir).join("installer.log");
    let content = std::fs::read_to_string(&log_path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let tail = if lines.len() > 40 {
        &lines[lines.len() - 40..]
    } else {
        &lines[..]
    };
    Some(tail.join("\n"))
}

// --- 3. ASEGURAR launcher_profiles.json PARA INSTALADORES FORGE/NEOFORGE ---
// Forge/NeoForge chequean que el directorio destino "parezca" una carpeta
// .minecraft real (donde ya corrió el launcher oficial), buscando este
// archivo. Si no existe, fallan con "there is no minecraft launcher
// profile...". No hace falta contenido real, un JSON vacío alcanza.
#[tauri::command]
pub async fn ensure_launcher_profile(instance_dir: String) -> Result<(), String> {
    let path = PathBuf::from(&instance_dir).join("launcher_profiles.json");
    if path.exists() {
        return Ok(());
    }
    tokio::fs::write(&path, "{}").await.map_err(|e| e.to_string())
}