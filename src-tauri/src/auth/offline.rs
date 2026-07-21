use crate::auth::models::AuthSession;

#[tauri::command]
pub fn offline_login(username: String) -> Result<AuthSession, String> {
    let username = username.trim().to_string();
    if !is_valid_minecraft_username(&username) {
        return Err("Nombre de usuario inválido".into());
    }
    Ok(AuthSession {
        uuid: offline_uuid(&username),
        username,
        access_token: "0".to_string(),
        user_type: "legacy".to_string(),
    })
}

fn is_valid_minecraft_username(username: &str) -> bool {
    let len = username.chars().count();
    len >= 3 && len <= 16 && username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn offline_uuid(username: &str) -> String {
    let mut bytes = md5::compute(format!("OfflinePlayer:{}", username)).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
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
