use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::mpsc;

pub enum Cmd {
    Update { title: String, artist: String, start_secs: i64 },
    Clear,
}

pub fn spawn(client_id: String) -> mpsc::Sender<Cmd> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    std::thread::spawn(move || run(client_id, rx));
    tx
}

fn run(client_id: String, rx: mpsc::Receiver<Cmd>) {
    let mut client = DiscordIpcClient::new(&client_id);
    let mut connected = client.connect().is_ok();

    for cmd in rx {
        if !connected {
            connected = client.connect().is_ok();
            if !connected {
                continue;
            }
        }

        match cmd {
            Cmd::Update { title, artist, start_secs } => {
                let state = if artist.is_empty() { "Noctune".to_string() } else { artist };
                let act = activity::Activity::new()
                    .details(&title)
                    .state(&state)
                    .activity_type(activity::ActivityType::Listening)
                    .timestamps(activity::Timestamps::new().start(start_secs));
                if client.set_activity(act).is_err() {
                    connected = false;
                }
            }
            Cmd::Clear => {
                let _ = client.clear_activity();
            }
        }
    }
    let _ = client.close();
}
