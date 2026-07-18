use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DISCORD_CLIENT_ID: &str = "1528082395746664538";

#[derive(Debug, Clone)]
pub enum DiscordCommand {
    UpdateActivity {
        details: String,
        state: String,
        large_image: Option<String>,
        large_text: Option<String>,
        small_image: Option<String>,
        small_text: Option<String>,
        start_timestamp: Option<i64>,
    },
    Clear,
    Shutdown,
}

#[derive(Clone)]
pub struct DiscordHandle {
    tx: Sender<DiscordCommand>,
}

impl DiscordHandle {
    pub fn send(&self, cmd: DiscordCommand) {
        let _ = self.tx.send(cmd);
    }
}

pub fn spawn_discord_worker() -> DiscordHandle {
    let (tx, rx) = channel::<DiscordCommand>();
    thread::spawn(move || discord_worker_loop(rx));
    DiscordHandle { tx }
}

fn discord_worker_loop(rx: Receiver<DiscordCommand>) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut last_cmd: Option<DiscordCommand> = None;

    loop {
        if client.is_none() {
            let mut c = DiscordIpcClient::new(DISCORD_CLIENT_ID);
            if c.connect().is_ok() {
                client = Some(c);
                if let Some(cmd) = last_cmd.clone() {
                    apply(&mut client, cmd);
                }
            }
        }

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(DiscordCommand::Shutdown) => {
                if let Some(mut c) = client.take() { let _ = c.close(); }
                break;
            }
            Ok(cmd) => {
                last_cmd = Some(cmd.clone());
                apply(&mut client, cmd);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn apply(client: &mut Option<DiscordIpcClient>, cmd: DiscordCommand) {
    let Some(c) = client.as_mut() else { return };

    let result = match cmd {
        DiscordCommand::Clear => c.clear_activity(),
        DiscordCommand::Shutdown => Ok(()),
        DiscordCommand::UpdateActivity { details, state, large_image, large_text, small_image, small_text, start_timestamp } => {
            let mut assets = activity::Assets::new();
            if let Some(ref img) = large_image { assets = assets.large_image(img); }
            if let Some(ref t) = large_text { assets = assets.large_text(t); }
            if let Some(ref img) = small_image { assets = assets.small_image(img); }
            if let Some(ref t) = small_text { assets = assets.small_text(t); }

            let mut act = activity::Activity::new()
                .details(&details)
                .state(&state)
                .assets(assets);

            if let Some(ts) = start_timestamp {
                act = act.timestamps(activity::Timestamps::new().start(ts));
            }
            c.set_activity(act)
        }
    };
    if result.is_err() {
        *client = None;
    }
}

pub fn now_ts() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}