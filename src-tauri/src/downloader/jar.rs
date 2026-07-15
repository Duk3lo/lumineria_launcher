use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tauri::Emitter;

#[tauri::command]
pub async fn execute_jar(window: tauri::Window, java_path: String, jar_path: String, args: Vec<String>, work_dir: String) -> Result<String, String> {
    let mut command = Command::new(&java_path);
    command.current_dir(&work_dir).arg("-jar").arg(&jar_path);
    for arg in &args { command.arg(arg); }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("Fallo: {}", e))?;
    let stdout_buf = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let stderr_buf = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let out_task = {
        let (window, buf, stdout) = (window.clone(), stdout_buf.clone(), child.stdout.take().unwrap());
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window.emit("process-log", serde_json::json!({ "stream": "stdout", "line": &line }));
                buf.lock().await.push(line);
            }
        })
    };

    let err_task = {
        let (window, buf, stderr) = (window.clone(), stderr_buf.clone(), child.stderr.take().unwrap());
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window.emit("process-log", serde_json::json!({ "stream": "stderr", "line": &line }));
                buf.lock().await.push(line);
            }
        })
    };

    let status = child.wait().await.unwrap();
    let _ = out_task.await;
    let _ = err_task.await;

    let stdout = stdout_buf.lock().await.join("\n");
    let stderr = stderr_buf.lock().await.join("\n");

    if status.success() { Ok(stdout) } else {
        let mut msg = format!("Error ejecutando jar.\n[stdout]\n{}\n[stderr]\n{}", stdout, stderr);
        if let Some(log) = read_installer_log_tail(&work_dir) { msg.push_str(&format!("\n[installer.log]\n{}", log)); }
        Err(msg)
    }
}

fn read_installer_log_tail(work_dir: &str) -> Option<String> {
    let content = std::fs::read_to_string(PathBuf::from(work_dir).join("installer.log")).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    Some((if lines.len() > 40 { &lines[lines.len() - 40..] } else { &lines[..] }).join("\n"))
}