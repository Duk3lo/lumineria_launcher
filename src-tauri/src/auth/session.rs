use crate::auth::microsoft::{ms_refresh_session, now_ts};
use crate::auth::models::{AccountStore, AuthSession};
use keyring::Entry;
use tauri::{AppHandle, Manager};

const SERVICE_NAME: &str = "Lumineria Launcher";
const LEGACY_ACCOUNTS_KEY: &str = "accounts";
const LEGACY_KEY: &str = "current_session";
const MARGEN_SEGUNDOS: i64 = 60;

fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, key).map_err(|e| e.to_string())
}


fn refresh_token_key(account_id: &str) -> String {
    format!("rt:{account_id}")
}

fn save_refresh_token(account_id: &str, refresh_token: &str) -> Result<(), String> {
    let e = entry(&refresh_token_key(account_id))?;
    if refresh_token.is_empty() {
        let _ = e.delete_credential();
        return Ok(());
    }
    e.set_password(refresh_token).map_err(|e| e.to_string())
}

fn load_refresh_token(account_id: &str) -> String {
    match entry(&refresh_token_key(account_id)) {
        Ok(e) => e.get_password().unwrap_or_default(),
        Err(_) => String::new(),
    }
}

fn delete_refresh_token(account_id: &str) {
    if let Ok(e) = entry(&refresh_token_key(account_id)) {
        let _ = e.delete_credential();
    }
}

fn accounts_meta_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("accounts.json"))
}

fn write_store(app: &AppHandle, store: &AccountStore) -> Result<(), String> {
    for account in &store.accounts {
        save_refresh_token(&account.account_id(), &account.refresh_token)?;
    }

    let mut sanitized = store.clone();
    for account in sanitized.accounts.iter_mut() {
        account.refresh_token = String::new();
    }

    let raw = serde_json::to_string_pretty(&sanitized).map_err(|e| e.to_string())?;
    std::fs::write(accounts_meta_path(app)?, raw).map_err(|e| e.to_string())
}

fn read_store(app: &AppHandle) -> Result<AccountStore, String> {
    let path = accounts_meta_path(app)?;
    if !path.exists() {
        return migrate_legacy(app);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut store: AccountStore = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    for account in store.accounts.iter_mut() {
        account.refresh_token = load_refresh_token(&account.account_id());
    }
    Ok(store)
}

fn migrate_legacy(app: &AppHandle) -> Result<AccountStore, String> {
    if let Ok(raw) = entry(LEGACY_ACCOUNTS_KEY)?.get_password() {
        if let Ok(store) = serde_json::from_str::<AccountStore>(&raw) {
            write_store(app, &store)?;
            let _ = entry(LEGACY_ACCOUNTS_KEY)?.delete_credential();
            return Ok(store);
        }
    }

    let legacy = entry(LEGACY_KEY)?;
    let raw = match legacy.get_password() {
        Ok(raw) => raw,
        Err(keyring::Error::NoEntry) => return Ok(AccountStore::default()),
        Err(e) => return Err(e.to_string()),
    };

    let session: AuthSession = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let store = AccountStore {
        active_id: Some(session.account_id()),
        accounts: vec![session],
    };
    write_store(app, &store)?;
    let _ = legacy.delete_credential();
    Ok(store)
}

impl AccountStore {
    fn position_of(&self, id: &str) -> Option<usize> {
        self.accounts.iter().position(|a| a.account_id() == id)
    }

    fn active(&self) -> Option<&AuthSession> {
        let id = self.active_id.as_deref()?;
        self.accounts.iter().find(|a| a.account_id() == id)
    }

    fn upsert(&mut self, session: AuthSession) {
        match self.position_of(&session.account_id()) {
            Some(i) => self.accounts[i] = session,
            None => self.accounts.push(session),
        }
    }

    fn normalize_active(&mut self) {
        let vigente = self
            .active_id
            .as_deref()
            .is_some_and(|id| self.position_of(id).is_some());
        if !vigente {
            self.active_id = self.accounts.first().map(|a| a.account_id());
        }
    }
}

#[tauri::command]
pub fn list_accounts(app: AppHandle) -> Result<AccountStore, String> {
    read_store(&app)
}

#[tauri::command]
pub fn add_account(app: AppHandle, session: AuthSession) -> Result<AccountStore, String> {
    let mut store = read_store(&app)?;
    store.active_id = Some(session.account_id());
    store.upsert(session);
    write_store(&app, &store)?;
    Ok(store)
}

#[tauri::command]
pub fn remove_account(app: AppHandle, id: String) -> Result<AccountStore, String> {
    let mut store = read_store(&app)?;
    store.accounts.retain(|a| a.account_id() != id);
    delete_refresh_token(&id);
    if store.active_id.as_deref() == Some(id.as_str()) {
        store.active_id = None;
    }
    store.normalize_active();
    write_store(&app, &store)?;
    Ok(store)
}

#[tauri::command]
pub async fn set_active_account(app: AppHandle, id: String) -> Result<AccountStore, String> {
    let mut store = read_store(&app)?;
    if store.position_of(&id).is_none() {
        return Err(format!("La cuenta {id} ya no existe"));
    }
    store.active_id = Some(id);
    write_store(&app, &store)?;
    refresh_active(&app, store).await
}

#[tauri::command]
pub async fn restore_accounts(app: AppHandle) -> Result<AccountStore, String> {
    let store = read_store(&app)?;
    refresh_active(&app, store).await
}

async fn refresh_active(app: &AppHandle, mut store: AccountStore) -> Result<AccountStore, String> {
    let session = match store.active() {
        Some(s) => s.clone(),
        None => return Ok(store),
    };

    if session.auth_flow == "offline" {
        return Ok(store);
    }
    if session.expires_at - MARGEN_SEGUNDOS > now_ts() {
        return Ok(store);
    }

    let id = session.account_id();
    if session.refresh_token.is_empty() {
        return drop_account(app, store, &id);
    }

    match ms_refresh_session(session.refresh_token, Some(session.auth_flow)).await {
        Ok(nueva) => {
            store.upsert(nueva);
            write_store(app, &store)?;
            Ok(store)
        }
        Err(_) => drop_account(app, store, &id),
    }
}

fn drop_account(app: &AppHandle, mut store: AccountStore, id: &str) -> Result<AccountStore, String> {
    store.accounts.retain(|a| a.account_id() != id);
    delete_refresh_token(id);
    if store.active_id.as_deref() == Some(id) {
        store.active_id = None;
    }
    store.normalize_active();
    write_store(app, &store)?;
    Ok(store)
}