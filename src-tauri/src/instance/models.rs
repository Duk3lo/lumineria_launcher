use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub filename: String,
    pub display_name: String,
    pub enabled: bool,
    pub size_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStatus {
    pub installed: bool,
    pub mods_count: u32,
}