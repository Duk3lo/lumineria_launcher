use anyhow::Result;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::process::Command as StdCommand;

use crate::net;

#[tauri::command]
pub async fn verify_and_get_java(version: u8, base_dir: String) -> Result<String, String> {
    match check_local_java(version, &base_dir) {
        JavaStatus::Ready(path) => Ok(path.to_string_lossy().to_string()),
        JavaStatus::Missing => Err("Java no está instalado".to_string()),
    }
}

#[tauri::command]
pub async fn download_java_command(version: u8, base_dir: String) -> Result<String, String> {
    download_jre(version, &base_dir).await.map_err(|e| format!("{:?}", e))?;
    Ok("Java descargado y extraído".to_string())
}

pub enum JavaStatus {
    Ready(PathBuf),
    Missing,
}

pub fn check_local_java(required_version: u8, base_dir: &str) -> JavaStatus {
    let runtime_dir = PathBuf::from(base_dir).join("runtimes").join(format!("jre-{}", required_version));
    if let Some(java_path) = find_executable(&runtime_dir) {
        if detect_java_major_version(&java_path.to_string_lossy()) == Some(required_version) {
            return JavaStatus::Ready(java_path);
        }
    }
    if detect_java_major_version("java") == Some(required_version) {
        return JavaStatus::Ready(PathBuf::from("java"));
    }

    JavaStatus::Missing
}

fn detect_java_major_version(java_bin: &str) -> Option<u8> {
    let mut cmd = StdCommand::new(java_bin);
    cmd.arg("-version");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first_line = stderr.lines().next()?;
    let version_str = first_line.split('"').nth(1)?;

    if let Some(rest) = version_str.strip_prefix("1.") {
        rest.split('.').next()?.parse().ok()
    } else {
        version_str.split('.').next()?.parse().ok()
    }
}

fn find_executable(base_dir: &Path) -> Option<PathBuf> {
    let target_name = if cfg!(target_os = "windows") { "java.exe" } else { "java" };
    if !base_dir.exists() { return None; }
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let bin_path = path.join("bin").join(target_name);
                if bin_path.exists() { return Some(bin_path); }
            }
        }
    }
    None
}

fn adoptium_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn archive_extension() -> &'static str {
    if cfg!(target_os = "windows") { "zip" } else { "tar.gz" }
}

async fn download_jre(version: u8, base_dir: &str) -> Result<()> {
    let os = adoptium_os();
    let url = format!("https://api.adoptium.net/v3/binary/latest/{}/ga/{}/x64/jre/hotspot/normal/eclipse", version, os);

    let runtimes_dir = PathBuf::from(base_dir).join("runtimes");
    tokio::fs::create_dir_all(&runtimes_dir).await?;

    let ext = archive_extension();
    let archive_path = runtimes_dir.join(format!("runtime_{}.{}", version, ext));
    let dest_dir = runtimes_dir.join(format!("jre-{}", version));
    if dest_dir.exists() {
        tokio::fs::remove_dir_all(&dest_dir).await.ok();
    }

    let response = net::download_client().get(&url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("Adoptium respondió {} para {}", response.status(), url);
    }

    let mut file = File::create(&archive_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    file.flush().await?;
    drop(file);

    if ext == "zip" {
        extract_zip(&archive_path.to_string_lossy(), &dest_dir.to_string_lossy()).await?;
    } else {
        extract_tar_gz(&archive_path.to_string_lossy(), &dest_dir.to_string_lossy()).await?;
    }

    tokio::fs::remove_file(&archive_path).await?;
    Ok(())
}

async fn extract_zip(zip_path: &str, dest_dir: &str) -> Result<()> {
    let zip_path = zip_path.to_string();
    let dest_dir = dest_dir.to_string();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => PathBuf::from(&dest_dir).join(path),
                None => continue,
            };

            if (*file.name()).ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() { std::fs::create_dir_all(p)?; }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if outpath.file_name().unwrap_or_default() == "java" {
                        let mut perms = std::fs::metadata(&outpath)?.permissions();
                        perms.set_mode(0o755);
                        std::fs::set_permissions(&outpath, perms)?;
                    }
                }
            }
        }
        Ok(())
    }).await??;
    Ok(())
}

async fn extract_tar_gz(archive_path: &str, dest_dir: &str) -> Result<()> {
    let archive_path = archive_path.to_string();
    let dest_dir = dest_dir.to_string();

    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&dest_dir)?;
        let file = std::fs::File::open(&archive_path)?;
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        archive.unpack(&dest_dir)?;
        Ok(())
    }).await??;
    Ok(())
}