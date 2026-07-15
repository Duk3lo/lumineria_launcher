use anyhow::Result;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::process::Command as StdCommand; 

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
    // Busca en: MiCarpeta/runtimes/jre-21
    let runtime_dir = PathBuf::from(base_dir).join("runtimes").join(format!("jre-{}", required_version));

    if let Some(java_path) = find_executable(&runtime_dir) {
        // ¡LA MEJORA! Ejecutamos "java -version" de forma oculta
        match StdCommand::new(&java_path).arg("-version").output() {
            Ok(output) => {
                if output.status.success() {
                    // Si el comando fue exitoso, Java funciona perfectamente
                    return JavaStatus::Ready(java_path);
                } else {
                    println!("Java existe pero está corrupto o falló al ejecutarse.");
                }
            },
            Err(e) => println!("Error al intentar ejecutar Java: {}", e),
        }
    }
    
    // Si no existe, o si falló la prueba de "-version", lo marcamos como faltante
    // para que el launcher lo descargue de nuevo automáticamente.
    JavaStatus::Missing
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

/// Nombre de sistema operativo tal como lo espera la API de Adoptium.
fn adoptium_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

/// Adoptium entrega .zip para Windows y .tar.gz para Linux/Mac —
/// hay que guardarlo y descomprimirlo con el formato correcto.
fn archive_extension() -> &'static str {
    if cfg!(target_os = "windows") { "zip" } else { "tar.gz" }
}

async fn download_jre(version: u8, base_dir: &str) -> Result<()> {
    let os = adoptium_os();
    let url = format!("https://api.adoptium.net/v3/binary/latest/{}/ga/{}/x64/jre/hotspot/normal/eclipse", version, os);

    let runtimes_dir = PathBuf::from(base_dir).join("runtimes");
    tokio::fs::create_dir_all(&runtimes_dir).await?; // Crea la carpeta runtimes si no existe

    let ext = archive_extension();
    let archive_path = runtimes_dir.join(format!("runtime_{}.{}", version, ext));
    let dest_dir = runtimes_dir.join(format!("jre-{}", version));

    // Si quedó una carpeta de un intento anterior fallido, la limpiamos
    // para no mezclar archivos de una extracción rota con la nueva.
    if dest_dir.exists() {
        tokio::fs::remove_dir_all(&dest_dir).await.ok();
    }

    let response = reqwest::get(&url).await?;
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
    // (TU CÓDIGO ACTUAL DE extract_zip SE QUEDA EXACTAMENTE IGUAL)
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

/// Extrae un .tar.gz (formato real que entrega Adoptium para Linux/Mac).
/// tar conserva los permisos originales del archivo, así que bin/java
/// ya queda ejecutable sin necesidad de un chmod manual.
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