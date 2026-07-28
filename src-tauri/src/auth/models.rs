use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub owns_minecraft: bool,
    #[serde(default = "default_auth_flow")]
    pub auth_flow: String,
}

impl AuthSession {
    pub fn account_id(&self) -> String {
        format!("{}:{}", self.user_type, self.uuid)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStore {
    #[serde(default)]
    pub accounts: Vec<AuthSession>,
    #[serde(default)]
    pub active_id: Option<String>,
}

fn default_auth_flow() -> String {
    "device_code".to_string()
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