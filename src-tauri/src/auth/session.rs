use crate::auth::microsoft::{ms_refresh_session, now_ts};
use crate::auth::models::{AccountStore, AuthSession};
use keyring::Entry;

const SERVICE_NAME: &str = "Lumineria Launcher";
const ACCOUNTS_KEY: &str = "accounts";
const LEGACY_KEY: &str = "current_session";
const MARGEN_SEGUNDOS: i64 = 60;

fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, key).map_err(|e| e.to_string())
}

fn write_store(store: &AccountStore) -> Result<(), String> {
    let raw = serde_json::to_string(store).map_err(|e| e.to_string())?;
    entry(ACCOUNTS_KEY)?
        .set_password(&raw)
        .map_err(|e| e.to_string())
}

fn read_store() -> Result<AccountStore, String> {
    match entry(ACCOUNTS_KEY)?.get_password() {
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| e.to_string()),
        Err(keyring::Error::NoEntry) => migrate_legacy(),
        Err(e) => Err(e.to_string()),
    }
}

fn migrate_legacy() -> Result<AccountStore, String> {
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
    write_store(&store)?;
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
pub fn list_accounts() -> Result<AccountStore, String> {
    read_store()
}

#[tauri::command]
pub fn add_account(session: AuthSession) -> Result<AccountStore, String> {
    let mut store = read_store()?;
    store.active_id = Some(session.account_id());
    store.upsert(session);
    write_store(&store)?;
    Ok(store)
}

#[tauri::command]
pub fn remove_account(id: String) -> Result<AccountStore, String> {
    let mut store = read_store()?;
    store.accounts.retain(|a| a.account_id() != id);
    if store.active_id.as_deref() == Some(id.as_str()) {
        store.active_id = None;
    }
    store.normalize_active();
    write_store(&store)?;
    Ok(store)
}

#[tauri::command]
pub async fn set_active_account(id: String) -> Result<AccountStore, String> {
    let mut store = read_store()?;
    if store.position_of(&id).is_none() {
        return Err(format!("La cuenta {id} ya no existe"));
    }
    store.active_id = Some(id);
    write_store(&store)?;
    refresh_active(store).await
}

#[tauri::command]
pub async fn restore_accounts() -> Result<AccountStore, String> {
    let store = read_store()?;
    refresh_active(store).await
}

async fn refresh_active(mut store: AccountStore) -> Result<AccountStore, String> {
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
        return drop_account(store, &id);
    }

    match ms_refresh_session(session.refresh_token, Some(session.auth_flow)).await {
        Ok(nueva) => {
            store.upsert(nueva);
            write_store(&store)?;
            Ok(store)
        }
        Err(_) => drop_account(store, &id),
    }
}

fn drop_account(mut store: AccountStore, id: &str) -> Result<AccountStore, String> {
    store.accounts.retain(|a| a.account_id() != id);
    if store.active_id.as_deref() == Some(id) {
        store.active_id = None;
    }
    store.normalize_active();
    write_store(&store)?;
    Ok(store)
}