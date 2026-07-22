use std::sync::OnceLock;
use crate::config::AppConfig;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();
static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
static DOWNLOAD: OnceLock<reqwest::Client> = OnceLock::new();

fn config() -> &'static AppConfig {
    CONFIG.get_or_init(AppConfig::from_env)
}

pub fn modpacks_api_url() -> &'static str {
    &config().modpacks_api_url
}

pub fn http_client() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        let cfg = config();
        reqwest::Client::builder()
            .connect_timeout(cfg.connect_timeout)
            .timeout(cfg.http_timeout)
            .build()
            .expect("no se pudo construir el cliente HTTP")
    })
}

pub fn download_client() -> &'static reqwest::Client {
    DOWNLOAD.get_or_init(|| {
        let cfg = config();
        reqwest::Client::builder()
            .connect_timeout(cfg.connect_timeout)
            .timeout(cfg.download_timeout)
            .build()
            .expect("no se pudo construir el cliente de descargas")
    })
}

#[tauri::command]
pub async fn check_url_reachable(url: String) -> bool {
    match http_client().get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub trait HideConsoleExt {
    fn hide_console(&mut self) -> &mut Self;
}

#[cfg(target_os = "windows")]
impl HideConsoleExt for std::process::Command {
    fn hide_console(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

#[cfg(target_os = "windows")]
impl HideConsoleExt for tokio::process::Command {
    fn hide_console(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

#[cfg(not(target_os = "windows"))]
impl HideConsoleExt for std::process::Command {
    fn hide_console(&mut self) -> &mut Self {
        self
    }
}

#[cfg(not(target_os = "windows"))]
impl HideConsoleExt for tokio::process::Command {
    fn hide_console(&mut self) -> &mut Self {
        self
    }
}
