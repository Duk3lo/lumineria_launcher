use std::sync::Arc;
use tokio::sync::Mutex;

use crate::discord;

#[derive(Debug, Clone, Default)]
pub struct RunningInstance {
    pub profile_id: String,
    pub title: String,
    pub loader_name: String,
    pub launched_at: i64,
    pub players_online: Option<i32>,
    pub max_players: Option<i32>,
    pub server_name: Option<String>,
    pub server_icon: Option<String>,
}

pub fn loader_presence_assets(loader_name: &str) -> (&'static str, String) {
    match loader_name.to_lowercase().as_str() {
        "neoforge" => ("neoforge", "NeoForge".to_string()),
        "forge" => ("forge", "Forge".to_string()),
        "fabric" => ("fabric", "Fabric".to_string()),
        "quilt" => ("quilt", "Quilt".to_string()),
        _ => ("vanilla", "Vanilla".to_string()),
    }
}

pub async fn register_instance(
    running_instances: &Arc<Mutex<Vec<RunningInstance>>>,
    discord: &discord::DiscordHandle,
    instance: RunningInstance,
) {
    {
        let mut instances = running_instances.lock().await;
        instances.retain(|i| i.profile_id != instance.profile_id);
        instances.push(instance);
    }
    refresh_discord_presence(running_instances, discord).await;
}

pub async fn unregister_instance(
    running_instances: &Arc<Mutex<Vec<RunningInstance>>>,
    discord: &discord::DiscordHandle,
    profile_id: &str,
) {
    {
        let mut instances = running_instances.lock().await;
        instances.retain(|i| i.profile_id != profile_id);
    }
    refresh_discord_presence(running_instances, discord).await;
}

pub async fn update_instance_status(
    running_instances: &Arc<Mutex<Vec<RunningInstance>>>,
    discord: &discord::DiscordHandle,
    profile_id: &str,
    players_online: i32,
    max_players: i32,
    server_name: Option<String>,
    server_icon: Option<String>,
) {
    {
        let mut instances = running_instances.lock().await;
        if let Some(instance) = instances.iter_mut().find(|i| i.profile_id == profile_id) {
            instance.players_online = Some(players_online);
            instance.max_players = Some(max_players);
            instance.server_name = server_name;
            instance.server_icon = server_icon;
        }
    }
    refresh_discord_presence(running_instances, discord).await;
}

async fn refresh_discord_presence(
    running_instances: &Arc<Mutex<Vec<RunningInstance>>>,
    discord: &discord::DiscordHandle,
) {
    let active = {
        let instances = running_instances.lock().await;
        instances.last().cloned()
    };

    match active {
        Some(instance) => {
            let (loader_img, loader_text) = loader_presence_assets(&instance.loader_name);
            let party_size = match (instance.players_online, instance.max_players) {
                (Some(online), Some(max)) if max > 0 => Some((online, max)),
                _ => None,
            };
            let large_image = "launcher_icon".to_string();
            let large_text = format!("Lumineria - {}", instance.title);
            let small_image = instance.server_icon.clone().unwrap_or_else(|| loader_img.to_string());
            let small_text = instance.server_name.clone().unwrap_or_else(|| loader_text.clone());

            let state_text = instance.server_name.clone().unwrap_or_else(|| "Jugando en Solitario".into());

            discord.send(discord::DiscordCommand::UpdateActivity {
                details: format!("Jugando {}", instance.title),
                state: state_text,
                large_image: Some(large_image),
                large_text: Some(large_text),
                small_image: Some(small_image),
                small_text: Some(small_text),
                start_timestamp: Some(instance.launched_at),
                party_size,
            });
        }
        None => {
            discord.send(discord::DiscordCommand::UpdateActivity {
                details: "Navegando por el Launcher".into(),
                state: "Preparando su próxima aventura".into(),
                large_image: Some("launcher_icon".into()),
                large_text: Some("Lumineria Launcher".into()),
                small_image: None,
                small_text: None,
                start_timestamp: Some(discord::now_ts()),
                party_size: None,
            });
        }
    }
}
