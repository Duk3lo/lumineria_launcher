use serde_json::json;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::auth::models::{AuthSession, DeviceCodeInfo};
use crate::net;
use crate::AppState;

const AZURE_CLIENT_ID: &str = "00000000402b5328";
const SCOPE: &str = "XboxLive.signin offline_access";

pub(crate) const FLOW_DEVICE_CODE: &str = "device_code";
pub(crate) const FLOW_LEGACY: &str = "legacy";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

pub(crate) fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut bytes = md5::compute(format!("lumineria-{}", nanos)).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub(crate) async fn read_json(
    resp: reqwest::Response,
    who: &str,
) -> Result<(reqwest::StatusCode, serde_json::Value), String> {
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("No se pudo leer la respuesta de {}: {}", who, e))?;
    if body.trim().is_empty() {
        return Ok((status, serde_json::Value::Null));
    }
    let value = serde_json::from_str(&body)
        .map_err(|_| format!("Respuesta inválida de {} (HTTP {})", who, status.as_u16()))?;
    Ok((status, value))
}

pub(crate) fn oauth_error(body: &serde_json::Value) -> Option<String> {
    let err = body["error"].as_str()?;
    let desc = body["error_description"].as_str().unwrap_or(err);
    if err == "unauthorized_client" || desc.contains("AADSTS700016") {
        return Some(format!(
            "Microsoft no reconoce el client_id '{}'.",
            AZURE_CLIENT_ID
        ));
    }
    Some(format!(
        "{} ({})",
        desc.lines().next().unwrap_or(desc).trim(),
        err
    ))
}

fn xsts_error(body: &serde_json::Value) -> String {
    match body["XErr"].as_i64().unwrap_or(0) {
        2148916233 => "Esta cuenta no tiene un perfil de Xbox Live.".into(),
        2148916238 => "Esta cuenta de menor necesita ser vinculada a una familia.".into(),
        _ => "XSTS rechazó la cuenta.".into(),
    }
}

#[tauri::command]
pub async fn ms_login_start() -> Result<DeviceCodeInfo, String> {
    let resp = net::http_client()
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", AZURE_CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| format!("Sin conexión con Microsoft: {}", e))?;
    let (_, body) = read_json(resp, "Microsoft").await?;
    if let Some(err) = oauth_error(&body) {
        return Err(err);
    }

    Ok(DeviceCodeInfo {
        device_code: body["device_code"]
            .as_str()
            .ok_or("Falta device_code")?
            .to_string(),
        user_code: body["user_code"]
            .as_str()
            .ok_or("Falta user_code")?
            .to_string(),
        verification_uri: body["verification_uri"]
            .as_str()
            .ok_or("Falta verification_uri")?
            .to_string(),
        interval: body["interval"].as_u64().unwrap_or(5),
        expires_in: body["expires_in"].as_u64().unwrap_or(900),
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
    let mut current_interval = interval.max(1);

    let (ms_access_token, refresh_token) = loop {
        if start.elapsed().as_secs() > expires_in {
            return Err("El código expiró.".into());
        }
        if state.ms_login_cancel.load(Ordering::SeqCst) {
            return Err("Inicio de sesión cancelado".into());
        }
        tokio::time::sleep(Duration::from_secs(current_interval)).await;
        if state.ms_login_cancel.load(Ordering::SeqCst) {
            return Err("Inicio de sesión cancelado".into());
        }

        let resp = net::http_client()
            .post(TOKEN_URL)
            .form(&[
                ("client_id", AZURE_CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &device_code),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let (_, body) = read_json(resp, "Microsoft").await?;

        match body["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                current_interval += 5;
                continue;
            }
            Some("expired_token") => return Err("El código expiró.".into()),
            Some("access_denied") => return Err("Rechazaste el acceso.".into()),
            Some(_) => return Err(oauth_error(&body).unwrap_or("Error desconocido".into())),
            None => {}
        }
        break (
            body["access_token"].as_str().unwrap().to_string(),
            body["refresh_token"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
    };
    minecraft_session(
        &format!("d={}", ms_access_token),
        refresh_token,
        FLOW_DEVICE_CODE,
    )
    .await
}

#[tauri::command]
pub async fn ms_refresh_session(
    refresh_token: String,
    auth_flow: Option<String>,
) -> Result<AuthSession, String> {
    if refresh_token.is_empty() {
        return Err("Iniciá sesión de nuevo.".into());
    }
    if auth_flow.as_deref() == Some(FLOW_LEGACY) {
        return super::microsoft_legacy::refresh_legacy(refresh_token).await;
    }

    let resp = net::http_client()
        .post(TOKEN_URL)
        .form(&[
            ("client_id", AZURE_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let (_, body) = read_json(resp, "Microsoft").await?;
    if let Some(err) = oauth_error(&body) {
        return Err(err);
    }

    let ms_access_token = body["access_token"].as_str().unwrap().to_string();
    let nuevo_refresh = body["refresh_token"]
        .as_str()
        .unwrap_or(&refresh_token)
        .to_string();
    minecraft_session(
        &format!("d={}", ms_access_token),
        nuevo_refresh,
        FLOW_DEVICE_CODE,
    )
    .await
}

pub(crate) async fn minecraft_session(
    rps_ticket: &str,
    refresh_token: String,
    auth_flow: &str,
) -> Result<AuthSession, String> {
    let resp = net::http_client().post("https://user.auth.xboxlive.com/user/authenticate").json(&json!({"Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": rps_ticket}, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"})).send().await.map_err(|e| e.to_string())?;
    let (status, xbl) = read_json(resp, "Xbox").await?;
    if !status.is_success() {
        return Err("Xbox Live rechazó la sesión.".into());
    }
    let xbl_token = xbl["Token"].as_str().unwrap();
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap();

    let resp = net::http_client().post("https://xsts.auth.xboxlive.com/xsts/authorize").json(&json!({"Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] }, "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"})).send().await.map_err(|e| e.to_string())?;
    let (status, xsts) = read_json(resp, "XSTS").await?;
    if !status.is_success() || xsts.get("XErr").is_some() {
        return Err(xsts_error(&xsts));
    }
    let xsts_token = xsts["Token"].as_str().unwrap();

    let resp = net::http_client()
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let (_, mc) = read_json(resp, "MC").await?;
    let mc_access_token = mc["access_token"].as_str().unwrap().to_string();
    let expires_in = mc["expires_in"].as_i64().unwrap_or(86_400);

    let owns_minecraft = fetch_owns_minecraft(&mc_access_token).await;
    let resp = net::http_client()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let (status, profile) = read_json(resp, "MC Profile").await?;

    if status.as_u16() == 404 {
        return Err(if owns_minecraft {
            "Cuenta premium sin perfil. Entrá a minecraft.net para crear tu nombre.".into()
        } else {
            "Esta cuenta no tiene comprado Minecraft.".into()
        });
    }

    Ok(AuthSession {
        username: profile["name"].as_str().unwrap().to_string(),
        uuid: profile["id"].as_str().unwrap().to_string(),
        access_token: mc_access_token,
        user_type: "msa".to_string(),
        refresh_token,
        expires_at: now_ts() + expires_in,
        owns_minecraft,
        auth_flow: auth_flow.to_string(),
    })
}

async fn fetch_owns_minecraft(mc_access_token: &str) -> bool {
    let url = format!(
        "https://api.minecraftservices.com/entitlements/license?requestId={}",
        request_id()
    );
    if let Ok(resp) = net::http_client()
        .get(&url)
        .bearer_auth(mc_access_token)
        .send()
        .await
    {
        if let Ok(body) = resp.json::<serde_json::Value>().await {
            return body["items"]
                .as_array()
                .map(|items| {
                    items.iter().any(|item| {
                        matches!(
                            item["name"].as_str(),
                            Some("product_minecraft") | Some("game_minecraft")
                        )
                    })
                })
                .unwrap_or(false);
        }
    }
    false
}
