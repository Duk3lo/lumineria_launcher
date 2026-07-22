use std::env;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub modpacks_api_url: String,
    pub http_timeout: Duration,
    pub download_timeout: Duration,
    pub connect_timeout: Duration,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            modpacks_api_url: "http://localhost:8000/modpacks.json".into(),
            http_timeout: Duration::from_secs(15),
            download_timeout: Duration::from_secs(120),
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            modpacks_api_url: env::var("LUMINERIA_MODPACKS_API_URL")
                .unwrap_or(default.modpacks_api_url),
            http_timeout: parse_secs_env("LUMINERIA_HTTP_TIMEOUT_SECS")
                .unwrap_or(default.http_timeout),
            download_timeout: parse_secs_env("LUMINERIA_DOWNLOAD_TIMEOUT_SECS")
                .unwrap_or(default.download_timeout),
            connect_timeout: parse_secs_env("LUMINERIA_CONNECT_TIMEOUT_SECS")
                .unwrap_or(default.connect_timeout),
        }
    }
}

fn parse_secs_env(key: &str) -> Option<Duration> {
    env::var(key).ok()?.parse::<u64>().ok().map(Duration::from_secs)
}