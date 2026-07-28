use crate::auth::microsoft::{minecraft_session, oauth_error, read_json, FLOW_LEGACY};
use crate::auth::models::AuthSession;
use crate::net;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const LEGACY_CLIENT_ID: &str = "00000000402b5328";
const LEGACY_AUTH_URL: &str = "https://login.live.com/oauth20_authorize.srf";
const LEGACY_TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
const LEGACY_REDIRECT: &str = "https://login.live.com/oauth20_desktop.srf";
const LEGACY_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

#[tauri::command]
pub async fn ms_legacy_login(app: AppHandle) -> Result<AuthSession, String> {
    let code = wait_for_code(&app).await?;
    let resp = net::http_client()
        .post(LEGACY_TOKEN_URL)
        .form(&[
            ("client_id", LEGACY_CLIENT_ID),
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", LEGACY_REDIRECT),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let (_, body) = read_json(resp, "Microsoft").await?;
    if let Some(err) = oauth_error(&body) {
        return Err(err);
    }

    let access_token = body["access_token"].as_str().unwrap().to_string();
    let refresh_token = body["refresh_token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    minecraft_session(&access_token, refresh_token, FLOW_LEGACY).await
}

pub async fn refresh_legacy(refresh_token: String) -> Result<AuthSession, String> {
    let resp = net::http_client()
        .post(LEGACY_TOKEN_URL)
        .form(&[
            ("client_id", LEGACY_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
            ("redirect_uri", LEGACY_REDIRECT),
            ("scope", LEGACY_SCOPE),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let (_, body) = read_json(resp, "Microsoft").await?;
    if let Some(err) = oauth_error(&body) {
        return Err(err);
    }

    let access_token = body["access_token"].as_str().unwrap().to_string();
    let nuevo_refresh = body["refresh_token"]
        .as_str()
        .unwrap_or(&refresh_token)
        .to_string();
    minecraft_session(&access_token, nuevo_refresh, FLOW_LEGACY).await
}

async fn wait_for_code(app: &AppHandle) -> Result<String, String> {
    let auth_url = reqwest::Url::parse_with_params(
        LEGACY_AUTH_URL,
        &[
            ("client_id", LEGACY_CLIENT_ID),
            ("response_type", "code"),
            ("scope", LEGACY_SCOPE),
            ("redirect_uri", LEGACY_REDIRECT),
            ("prompt", "select_account"),
        ],
    )
    .unwrap();
    let url = tauri::Url::parse(auth_url.as_str()).unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let sender = Arc::new(Mutex::new(Some(tx)));
    let sender_nav = sender.clone();

    let window = WebviewWindowBuilder::new(app, "ms-legacy-login", WebviewUrl::External(url))
        .title("Iniciar sesión con Microsoft")
        .inner_size(520.0, 720.0)
        .center()
        .on_navigation(move |url| {
            if !url.as_str().starts_with(LEGACY_REDIRECT) {
                return true;
            }
            let mut code = None;
            let mut error = None;
            for (clave, valor) in url.query_pairs() {
                match clave.as_ref() {
                    "code" => code = Some(valor.into_owned()),
                    "error" | "error_description" => error = Some(valor.into_owned()),
                    _ => {}
                }
            }
            let res = match (code, error) {
                (Some(c), _) => Ok(c),
                (None, Some(e)) => Err(e),
                _ => Err("Sin código".into()),
            };
            if let Ok(mut g) = sender_nav.lock() {
                if let Some(tx) = g.take() {
                    let _ = tx.send(res);
                }
            }
            false
        })
        .build()
        .unwrap();

    let sender_close = sender.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
        ) {
            if let Ok(mut g) = sender_close.lock() {
                if let Some(tx) = g.take() {
                    let _ = tx.send(Err("Ventana cerrada".into()));
                }
            }
        }
    });

    let result = match tokio::time::timeout(Duration::from_secs(600), rx).await {
        Ok(Ok(Ok(res))) => Ok(res),
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => Err("Ventana cerrada inesperadamente".into()),
        Err(_) => Err("Tiempo agotado".into()),
    };

    let _ = window.close();
    result
}
