use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
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
