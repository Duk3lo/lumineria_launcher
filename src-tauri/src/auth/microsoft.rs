use serde_json::json;
use std::time::Duration;
use crate::auth::models::{AuthSession, DeviceCodeInfo};

const CLIENT_ID: &str = "TU_CLIENT_ID_DE_AZURE_AQUI";

#[tauri::command]
pub async fn ms_login_start() -> Result<DeviceCodeInfo, String> {
    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", CLIENT_ID), ("scope", "XboxLive.signin offline_access")])
        .send().await.unwrap().json().await.unwrap();

    Ok(DeviceCodeInfo {
        device_code: resp["device_code"].as_str().unwrap_or_default().to_string(),
        user_code: resp["user_code"].as_str().unwrap_or_default().to_string(),
        verification_uri: resp["verification_uri"].as_str().unwrap_or_default().to_string(),
        interval: resp["interval"].as_u64().unwrap_or(5),
        expires_in: resp["expires_in"].as_u64().unwrap_or(900),
    })
}

#[tauri::command]
pub async fn ms_login_poll(device_code: String, interval: u64, expires_in: u64) -> Result<AuthSession, String> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();

    let ms_access_token = loop {
        if start.elapsed().as_secs() > expires_in { return Err("Código expirado".into()); }
        tokio::time::sleep(Duration::from_secs(interval)).await;

        let resp: serde_json::Value = client.post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[("client_id", CLIENT_ID), ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("device_code", &device_code)])
            .send().await.unwrap().json().await.unwrap();

        if let Some(token) = resp["access_token"].as_str() { break token.to_string(); }
        if let Some(err) = resp["error"].as_str() { if err != "authorization_pending" { return Err(err.into()); } }
    };

    let xbl: serde_json::Value = client.post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({ "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", ms_access_token) }, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT" }))
        .send().await.unwrap().json().await.unwrap();

    let xbl_token = xbl["Token"].as_str().unwrap();
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap();

    let xsts: serde_json::Value = client.post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({ "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] }, "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT" }))
        .send().await.unwrap().json().await.unwrap();

    if xsts.get("XErr").is_some() { return Err("Cuenta no autorizada".into()); }
    let xsts_token = xsts["Token"].as_str().unwrap();

    let mc: serde_json::Value = client.post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }))
        .send().await.unwrap().json().await.unwrap();
    
    let mc_access_token = mc["access_token"].as_str().unwrap().to_string();

    let profile: serde_json::Value = client.get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_access_token).send().await.unwrap().json().await.unwrap();

    Ok(AuthSession {
        username: profile["name"].as_str().unwrap().to_string(),
        uuid: profile["id"].as_str().unwrap_or_default().to_string(),
        access_token: mc_access_token,
        user_type: "msa".to_string(),
    })
}