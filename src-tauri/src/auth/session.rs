use crate::auth::models::AuthSession;
use keyring::Entry;

const SERVICE_NAME: &str = "Lumineria Launcher";
const ACCOUNT_KEY: &str = "current_session";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, ACCOUNT_KEY).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_session(base_dir: String, session: AuthSession) -> Result<(), String> {
    let _ = base_dir; // ya no se usa: el token vive en el almacén seguro del SO
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