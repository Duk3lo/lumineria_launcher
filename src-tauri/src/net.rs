use std::sync::OnceLock;
use std::time::Duration;

pub const DEFAULT_MODPACKS_API_URL: &str = "http://localhost:8000/modpacks.json";

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
static DOWNLOAD: OnceLock<reqwest::Client> = OnceLock::new();

pub fn http_client() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .expect("no se pudo construir el cliente HTTP")
    })
}

pub fn download_client() -> &'static reqwest::Client {
    DOWNLOAD.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("no se pudo construir el cliente de descargas")
    })
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
