use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

#[tauri::command]
pub async fn download_generic_file(url: String, dest_path: String) -> Result<String, String> {
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() { return Err(format!("Error {} descargando", response.status())); }
    
    let mut file = tokio::fs::File::create(&dest_path).await.map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk.map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
    }
    Ok(dest_path)
}