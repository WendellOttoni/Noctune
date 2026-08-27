pub mod api;
pub mod native;
pub mod oauth;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use api::SpotifyApi;
pub use native::NativeSpotifySession;
#[allow(unused_imports)]
pub use oauth::AuthSession;
pub use oauth::{authorize, exchange_code, refresh_token};

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

const SECRETS_SERVICE: &str = "spotify";
const SECRETS_KEY: &str = "tokens";

/// Legacy file path — only used for one-time migration into the keyring (#98).
pub fn tokens_path() -> Result<PathBuf> {
    Ok(crate::config::project_dirs()?
        .config_dir()
        .join("spotify-tokens.json"))
}

pub fn load_tokens() -> Option<Tokens> {
    // Pull any legacy file into keyring before reading. After migration the
    // file no longer exists and this is a cheap no-op.
    if let Ok(legacy) = tokens_path() {
        crate::secrets::migrate_from_file(SECRETS_SERVICE, SECRETS_KEY, &legacy);
    }
    let text = crate::secrets::load(SECRETS_SERVICE, SECRETS_KEY)?;
    serde_json::from_str(&text).ok()
}

pub fn save_tokens(tokens: &Tokens) -> Result<()> {
    let text = serde_json::to_string(tokens)?;
    crate::secrets::store(SECRETS_SERVICE, SECRETS_KEY, &text);
    Ok(())
}

#[allow(dead_code)]
pub fn delete_tokens() {
    crate::secrets::delete(SECRETS_SERVICE, SECRETS_KEY);
}
