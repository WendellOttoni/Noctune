use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub music_dirs: Vec<PathBuf>,
    pub theme: String,
    pub keybinds: Keybinds,
    pub playback: Playback,
    #[serde(default)]
    pub spotify: SpotifyConfig,
    #[serde(default)]
    pub visualizer: VisualizerConfig,
    #[serde(default)]
    pub lastfm: LastfmConfig,
    #[serde(default)]
    pub discord: DiscordConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizerConfig {
    pub sensitivity: f32,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self { sensitivity: 1.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyConfig {
    pub client_id: String,
    pub redirect_port: u16,
}

impl Default for SpotifyConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            redirect_port: 8888,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastfmConfig {
    pub api_key: String,
    pub api_secret: String,
}

impl Default for LastfmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
        }
    }
}

impl LastfmConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.api_secret.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub client_id: String,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self { client_id: String::new() }
    }
}

impl DiscordConfig {
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }
}

impl SpotifyConfig {
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.redirect_port)
    }
    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinds {
    pub quit: String,
    pub play_pause: String,
    pub next: String,
    pub prev: String,
    pub volume_up: String,
    pub volume_down: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playback {
    pub default_volume: f32,
    pub shuffle: bool,
    pub repeat: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_dirs: default_music_dirs(),
            theme: "default".to_string(),
            keybinds: Keybinds {
                quit: "q".into(),
                play_pause: "space".into(),
                next: "n".into(),
                prev: "p".into(),
                volume_up: "+".into(),
                volume_down: "-".into(),
            },
            playback: Playback {
                default_volume: 0.7,
                shuffle: false,
                repeat: false,
            },
            spotify: SpotifyConfig::default(),
            visualizer: VisualizerConfig::default(),
            lastfm: LastfmConfig::default(),
            discord: DiscordConfig::default(),
        }
    }
}

impl Config {
    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load_or_default() -> Result<Self> {
        let path = config_path()?;
        if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            Ok(toml::from_str(&text)?)
        } else {
            let cfg = Self::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&path, toml::to_string_pretty(&cfg)?).ok();
            Ok(cfg)
        }
    }
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "noctune", "noctune")
        .context("could not determine config directory")
}

pub fn config_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

pub fn themes_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("themes"))
}

pub fn playlists_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("playlists"))
}

fn default_music_dirs() -> Vec<PathBuf> {
    directories::UserDirs::new()
        .and_then(|u| u.audio_dir().map(|p| p.to_path_buf()))
        .map(|p| vec![p])
        .unwrap_or_default()
}
