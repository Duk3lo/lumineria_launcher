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