use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, BufReader};
use serde::Deserialize;
use crate::discord::{DiscordHandle, DiscordCommand, now_ts};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModEvent {
    ServerJoin { host: String, port: u16 },
    ServerLeave,
}

pub async fn start_ipc_bridge(discord: DiscordHandle) -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                let discord = discord.clone();
                tokio::spawn(handle_connection(socket, discord));
            }
        }
    });

    Ok(port)
}

async fn handle_connection(socket: TcpStream, discord: DiscordHandle) {
    let mut reader = BufReader::new(socket);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                if let Ok(event) = serde_json::from_str::<ModEvent>(line.trim()) {
                    handle_event(event, &discord).await;
                }
            }
            Err(_) => break,
        }
    }
}

async fn handle_event(event: ModEvent, discord: &DiscordHandle) {
    match event {
        ModEvent::ServerJoin { host, port } => {
            let (state_text, large_text) = match ping_server(&host, port).await {
                Some((motd, _favicon)) => (motd, format!("{}:{}", host, port)),
                None => ("Conectado a un servidor".to_string(), format!("{}:{}", host, port)),
            };

            discord.send(DiscordCommand::UpdateActivity {
                details: "Jugando en un servidor".into(),
                state: state_text,
                large_image: Some("server_default".into()),
                large_text: Some(large_text),
                small_image: Some("launcher_icon".into()),
                small_text: Some("Lumineria Launcher".into()),
                start_timestamp: Some(now_ts()),
            });
        }
        ModEvent::ServerLeave => {
            discord.send(DiscordCommand::UpdateActivity {
                details: "En el menú principal".into(),
                state: String::new(),
                large_image: Some("launcher_icon".into()),
                large_text: None,
                small_image: None,
                small_text: None,
                start_timestamp: Some(now_ts()),
            });
        }
    }
}

async fn ping_server(host: &str, port: u16) -> Option<(String, Vec<u8>)> {
    let mut stream = TcpStream::connect((host, port)).await.ok()?;
    let pong = craftping::tokio::ping(&mut stream, host, port).await.ok()?;
    let motd = pong.description.map(|d| extract_motd(&d)).unwrap_or_default();
    let favicon = pong.favicon.unwrap_or_default();
    Some((motd, favicon))
}

fn extract_motd(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(_) => {
            let mut out = String::new();
            if let Some(t) = value.get("text").and_then(|t| t.as_str()) {
                out.push_str(t);
            }
            if let Some(extra) = value.get("extra").and_then(|e| e.as_array()) {
                for part in extra {
                    out.push_str(&extract_motd(part));
                }
            }
            out
        }
        _ => String::new(),
    }
}