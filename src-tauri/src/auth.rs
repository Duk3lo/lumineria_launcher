use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

/// Reemplazá esto por el "Application (client) ID" de tu propia app
/// registrada en https://portal.azure.com -> App registrations -> New registration.
/// - Supported account types: "Personal Microsoft accounts only"
/// - En Authentication, activá "Allow public client flows" (lo necesita el device code flow)
/// - No hace falta client secret ni redirect URI para este flujo.
const CLIENT_ID: &str = "TU_CLIENT_ID_DE_AZURE_AQUI";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    /// "msa" para cuentas premium, "legacy" para offline
    pub user_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[tauri::command]
pub async fn ms_login_start() -> Result<DeviceCodeInfo, String> {
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", CLIENT_ID), ("scope", "XboxLive.signin offline_access")])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(DeviceCodeInfo {
        device_code: resp["device_code"].as_str().unwrap_or_default().to_string(),
        user_code: resp["user_code"].as_str().unwrap_or_default().to_string(),
        verification_uri: resp["verification_uri"].as_str().unwrap_or_default().to_string(),
        interval: resp["interval"].as_u64().unwrap_or(5),
        expires_in: resp["expires_in"].as_u64().unwrap_or(900),
    })
}

#[tauri::command]
pub async fn ms_login_poll(
    device_code: String,
    interval: u64,
    expires_in: u64,
) -> Result<AuthSession, String> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();

    let ms_access_token = loop {
        if start.elapsed().as_secs() > expires_in {
            return Err("El código expiró, intenta iniciar sesión de nuevo".into());
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;

        let resp: serde_json::Value = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", device_code.as_str()),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        if let Some(token) = resp["access_token"].as_str() {
            break token.to_string();
        }
        if let Some(err) = resp["error"].as_str() {
            if err != "authorization_pending" {
                return Err(format!("Error de autenticación: {}", err));
            }
        }
    };

    // Xbox Live
    let xbl: serde_json::Value = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_access_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let xbl_token = xbl["Token"].as_str().ok_or("Fallo autenticación Xbox Live")?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or("uhs no encontrado en respuesta de Xbox Live")?;

    // XSTS
    let xsts: serde_json::Value = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    if xsts.get("XErr").is_some() {
        return Err(
            "Esta cuenta no puede usarse (revisa si tiene Xbox Live habilitado o es cuenta infantil)"
                .into(),
        );
    }
    let xsts_token = xsts["Token"].as_str().ok_or("Fallo autenticación XSTS")?;

    // Minecraft Services
    let mc: serde_json::Value = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }))
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let mc_access_token = mc["access_token"]
        .as_str()
        .ok_or("Fallo login de Minecraft Services")?
        .to_string();

    // Perfil (nombre + uuid) - también confirma que la cuenta es dueña de Java Edition
    let profile: serde_json::Value = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_access_token)
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let username = profile["name"]
        .as_str()
        .ok_or("Esta cuenta no tiene Minecraft Java Edition asociado")?
        .to_string();
    let uuid = profile["id"].as_str().unwrap_or_default().to_string();

    Ok(AuthSession {
        username,
        uuid,
        access_token: mc_access_token,
        user_type: "msa".to_string(),
    })
}

#[tauri::command]
pub fn offline_login(username: String) -> Result<AuthSession, String> {
    let username = username.trim().to_string();
    if !is_valid_minecraft_username(&username) {
        return Err(
            "Nombre de usuario inválido: usá entre 3 y 16 caracteres (letras, números y guion bajo, sin espacios)"
                .into(),
        );
    }
    Ok(AuthSession {
        uuid: offline_uuid(&username),
        username,
        access_token: "0".to_string(), // los servidores en modo offline no lo validan
        user_type: "legacy".to_string(),
    })
}

/// Un nombre de usuario válido para Minecraft: 3 a 16 caracteres,
/// solo letras, números y "_" (mismo criterio que usa Mojang para cuentas reales).
fn is_valid_minecraft_username(username: &str) -> bool {
    let len = username.chars().count();
    if len < 3 || len > 16 {
        return false;
    }
    username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Mismo algoritmo que usa el propio Minecraft/Java para cuentas offline:
/// UUID.nameUUIDFromBytes(("OfflinePlayer:" + username).getBytes(UTF_8))
fn offline_uuid(username: &str) -> String {
    let digest = md5::compute(format!("OfflinePlayer:{}", username));
    let mut bytes = digest.0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // versión 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variante RFC 4122
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]
    )
}

// ------------------------------------------------------------------
// Persistencia de sesión: guarda la última cuenta usada (nombre, uuid,
// tipo) en session.json dentro de la carpeta base del launcher, para
// que la próxima vez que se abra no haga falta loguearse de nuevo.
// ------------------------------------------------------------------

fn session_path(base_dir: &str) -> PathBuf {
    PathBuf::from(base_dir).join("session.json")
}

#[tauri::command]
pub async fn save_session(base_dir: String, session: AuthSession) -> Result<(), String> {
    let path = session_path(&base_dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, raw).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_session(base_dir: String) -> Result<Option<AuthSession>, String> {
    let path = session_path(&base_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| e.to_string())?;

    // Si el archivo está corrupto o es de un formato viejo, simplemente
    // lo ignoramos en vez de romper el arranque del launcher.
    Ok(serde_json::from_str::<AuthSession>(&raw).ok())
}

#[tauri::command]
pub async fn clear_session(base_dir: String) -> Result<(), String> {
    let path = session_path(&base_dir);
    if path.exists() {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}