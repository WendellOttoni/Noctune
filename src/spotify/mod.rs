pub mod api;
pub mod oauth;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub use api::SpotifyApi;
pub use oauth::{authorize, exchange_code, refresh_token};
#[allow(unused_imports)]
pub use oauth::AuthSession;

pub const REQUIRED_SCOPES: &[&str] = &[
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "playlist-read-private",
    "user-library-read",
    "streaming",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_unix: u64,
    pub scope: Option<String>,
    pub token_type: Option<String>,
}

impl Tokens {
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now + 30 >= self.expires_at_unix
    }
}

pub fn tokens_path() -> Result<PathBuf> {
    Ok(crate::config::project_dirs()?
        .config_dir()
        .join("spotify-tokens.json"))
}

pub fn load_tokens() -> Option<Tokens> {
    let path = tokens_path().ok()?;
    let text = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_tokens(tokens: &Tokens) -> Result<()> {
    let path = tokens_path()?;
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("spotify: failed to create dir {}: {e}", parent.display());
        }
    }
    fs::write(&path, serde_json::to_string_pretty(tokens)?)?;
    Ok(())
}
