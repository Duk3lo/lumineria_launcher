use serde_json::json;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::State;

use crate::auth::models::{AuthSession, DeviceCodeInfo};
use crate::net;
use crate::AppState;

const CLIENT_ID: &str = "00000000402b5328";

#[tauri::command]
pub async fn ms_login_start() -> Result<DeviceCodeInfo, String> {
    let resp: serde_json::Value = net::http_client()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", CLIENT_ID),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await
        .map_err(|e| format!("Sin conexión con Microsoft: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Respuesta inválida de Microsoft: {}", e))?;

    if let Some(err) = resp["error"].as_str() {
        let desc = resp["error_description"].as_str().unwrap_or(err);
        return Err(format!("{}: {}", err, desc));
    }

    let device_code = resp["device_code"]
        .as_str()
        .ok_or("Microsoft no devolvió device_code")?
        .to_string();
    let user_code = resp["user_code"]
        .as_str()
        .ok_or("Microsoft no devolvió user_code")?
        .to_string();
    let verification_uri = resp["verification_uri"]
        .as_str()
        .ok_or("Microsoft no devolvió verification_uri")?
        .to_string();

    Ok(DeviceCodeInfo {
        device_code,
        user_code,
        verification_uri,
        interval: resp["interval"].as_u64().unwrap_or(5),
        expires_in: resp["expires_in"].as_u64().unwrap_or(900),
    })
}

#[tauri::command]
pub fn cancel_ms_login(state: State<'_, AppState>) -> Result<(), String> {
    state.ms_login_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn ms_login_poll(
    device_code: String,
    interval: u64,
    expires_in: u64,
    state: State<'_, AppState>,
) -> Result<AuthSession, String> {
    state.ms_login_cancel.store(false, Ordering::SeqCst);
    let start = std::time::Instant::now();

    // --- 1. Poll hasta obtener el access_token de Microsoft ---
    let ms_access_token = loop {
        if start.elapsed().as_secs() > expires_in {
            return Err("Código expirado".into());
        }
        if state.ms_login_cancel.load(Ordering::SeqCst) {
            return Err("Inicio de sesión cancelado".into());
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if state.ms_login_cancel.load(Ordering::SeqCst) {
            return Err("Inicio de sesión cancelado".into());
        }

        let resp: serde_json::Value = net::http_client()
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &device_code),
            ])
            .send()
            .await
            .map_err(|e| format!("Sin conexión con Microsoft: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Respuesta inválida de Microsoft: {}", e))?;

        if let Some(token) = resp["access_token"].as_str() {
            break token.to_string();
        }
        if let Some(err) = resp["error"].as_str() {
            if err != "authorization_pending" {
                return Err(err.into());
            }
        }
    };

    // --- 2. Xbox Live ---
    let xbl: serde_json::Value = net::http_client()
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
        .send()
        .await
        .map_err(|e| format!("Sin conexión con Xbox Live: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Respuesta inválida de Xbox Live: {}", e))?;

    let xbl_token = xbl["Token"]
        .as_str()
        .ok_or("Xbox Live no devolvió un token válido")?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or("Xbox Live no devolvió el uhs esperado")?;

    // --- 3. XSTS ---
    let xsts: serde_json::Value = net::http_client()
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|e| format!("Sin conexión con XSTS: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Respuesta inválida de XSTS: {}", e))?;

    if xsts.get("XErr").is_some() {
        return Err("Cuenta no autorizada (probablemente no tiene Minecraft o es de una región restringida)".into());
    }
    let xsts_token = xsts["Token"]
        .as_str()
        .ok_or("XSTS no devolvió un token válido")?;

    // --- 4. Login en Minecraft Services ---
    let mc: serde_json::Value = net::http_client()
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }))
        .send()
        .await
        .map_err(|e| format!("Sin conexión con Minecraft Services: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Respuesta inválida de Minecraft Services: {}", e))?;

    let mc_access_token = mc["access_token"]
        .as_str()
        .ok_or("Minecraft Services no devolvió un access_token")?
        .to_string();

    // --- 5. Perfil del jugador ---
    let profile: serde_json::Value = net::http_client()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_access_token)
        .send()
        .await
        .map_err(|e| format!("Sin conexión al perfil de Minecraft: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Respuesta inválida del perfil de Minecraft: {}", e))?;

    let username = profile["name"]
        .as_str()
        .ok_or("Esta cuenta no tiene Minecraft asociado")?
        .to_string();

    Ok(AuthSession {
        username,
        uuid: profile["id"].as_str().unwrap_or_default().to_string(),
        access_token: mc_access_token,
        user_type: "msa".to_string(),
    })
}
