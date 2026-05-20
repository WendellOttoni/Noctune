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
    #[serde(default)]
    pub ytdlp: YtdlpConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub library: LibraryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtdlpConfig {
    /// Max attempts per yt-dlp invocation. 1 = no retry. (#69)
    pub max_retries: u32,
    /// Initial backoff in seconds; doubled each attempt.
    pub backoff_secs: u64,
    /// Backoff used after a 429 / rate-limit response.
    pub ratelimit_backoff_secs: u64,
}

impl Default for YtdlpConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_secs: 2,
            ratelimit_backoff_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 0 = no limit. (#70)
    pub max_size_mb: u64,
    /// Delete entries older than N days. 0 = never expire.
    pub expire_days: u64,
    pub album_art_max_mb: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { max_size_mb: 500, expire_days: 30, album_art_max_mb: 100 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    pub max_entries: usize,
    /// 0 = keep forever.
    pub retain_days: u64,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { max_entries: 1000, retain_days: 90 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    /// Watch music_dirs for filesystem changes (#72).
    pub watch_for_changes: bool,
    pub watch_debounce_ms: u64,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self { watch_for_changes: true, watch_debounce_ms: 500 }
    }
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
            ytdlp: YtdlpConfig::default(),
            cache: CacheConfig::default(),
            history: HistoryConfig::default(),
            library: LibraryConfig::default(),
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

pub fn eq_presets_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("eq_presets.toml"))
}

pub fn profiles_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("profiles.toml"))
}

// ── EQ presets ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    pub low_db: f32,
    pub mid_db: f32,
    pub high_db: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EqPresets {
    #[serde(default)]
    pub presets: Vec<EqPreset>,
}

impl EqPresets {
    pub fn load() -> Self {
        let Ok(path) = eq_presets_path() else { return Self::default() };
        let Ok(text) = std::fs::read_to_string(&path) else { return Self::default() };
        toml::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = eq_presets_path()?;
        if let Some(p) = path.parent() { std::fs::create_dir_all(p).ok(); }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

// ── Profiles ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub theme: String,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub eq_low_db: f32,
    pub eq_mid_db: f32,
    pub eq_high_db: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profiles {
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl Profiles {
    pub fn load() -> Self {
        let Ok(path) = profiles_path() else { return Self::default() };
        let Ok(text) = std::fs::read_to_string(&path) else { return Self::default() };
        toml::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = profiles_path()?;
        if let Some(p) = path.parent() { std::fs::create_dir_all(p).ok(); }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

fn default_music_dirs() -> Vec<PathBuf> {
    directories::UserDirs::new()
        .and_then(|u| u.audio_dir().map(|p| p.to_path_buf()))
        .map(|p| vec![p])
        .unwrap_or_default()
}
