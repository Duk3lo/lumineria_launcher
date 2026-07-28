use crate::auth::microsoft::{ms_refresh_session, now_ts};
use crate::auth::models::AuthSession;
use keyring::Entry;

const SERVICE_NAME: &str = "Lumineria Launcher";
const ACCOUNT_KEY: &str = "current_session";
const MARGEN_SEGUNDOS: i64 = 60;

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, ACCOUNT_KEY).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_session(base_dir: String, session: AuthSession) -> Result<(), String> {
    let _ = base_dir;
    let raw = serde_json::to_string(&session).map_err(|e| e.to_string())?;
    entry()?.set_password(&raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_session(base_dir: String) -> Result<Option<AuthSession>, String> {
    let _ = base_dir;
    match entry()?.get_password() {
        Ok(raw) => {
            let session = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            Ok(Some(session))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn clear_session(base_dir: String) -> Result<(), String> {
    let _ = base_dir;
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn restore_session(base_dir: String) -> Result<Option<AuthSession>, String> {
    let session = match load_session(base_dir.clone())? {
        Some(s) => s,
        None => return Ok(None),
    };

    if session.auth_flow == "offline" {
        return Ok(Some(session));
    }

    if session.expires_at - MARGEN_SEGUNDOS > now_ts() {
        return Ok(Some(session));
    }

    if session.refresh_token.is_empty() {
        let _ = clear_session(base_dir);
        return Ok(None);
    }

    match ms_refresh_session(session.refresh_token.clone(), Some(session.auth_flow.clone())).await
    {
        Ok(nueva) => {
            save_session(base_dir, nueva.clone())?;
            Ok(Some(nueva))
        }
        Err(_) => {
            let _ = clear_session(base_dir);
            Ok(None)
        }
    }
}