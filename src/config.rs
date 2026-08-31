use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    #[serde(default)]
    pub subsonic: SubsonicConfig,
    #[serde(default)]
    pub vault: VaultConfig,
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
        Self {
            max_size_mb: 500,
            expire_days: 30,
            album_art_max_mb: 100,
        }
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
        Self {
            max_entries: 1000,
            retain_days: 90,
        }
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
        Self {
            watch_for_changes: true,
            watch_debounce_ms: 500,
        }
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastfmConfig {
    pub api_key: String,
    pub api_secret: String,
}

impl LastfmConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.api_secret.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordConfig {
    pub client_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubsonicConfig {
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub enabled: bool,
}

impl SubsonicConfig {
    pub fn is_configured(&self) -> bool {
        !self.server_url.is_empty() && !self.username.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_vault_url")]
    pub server_url: String,
    #[serde(default)]
    pub upload_enabled: bool,
}

fn default_vault_url() -> String {
    "https://vault.noctune.dev".into()
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: default_vault_url(),
            upload_enabled: false,
        }
    }
}

impl VaultConfig {
    pub fn is_configured(&self) -> bool {
        !self.server_url.is_empty()
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
    // #67: optional bindings — empty string means "fall back to the built-in default".
    #[serde(default)]
    pub seek_back: String,
    #[serde(default)]
    pub seek_forward: String,
    #[serde(default)]
    pub stop: String,
    #[serde(default)]
    pub shuffle: String,
    #[serde(default)]
    pub repeat: String,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub tab: String,
    #[serde(default)]
    pub enqueue: String,
    #[serde(default)]
    pub remove_from_queue: String,
    #[serde(default)]
    pub clear_queue: String,
    #[serde(default)]
    pub toggle_mini: String,
    #[serde(default)]
    pub toggle_view: String,
    #[serde(default)]
    pub rescan: String,
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub cycle_theme: String,
    #[serde(default)]
    pub open_url: String,
    #[serde(default)]
    pub toggle_favorite: String,
    #[serde(default)]
    pub command_palette: String,
    #[serde(default)]
    pub subsonic_browser: String,
    #[serde(default)]
    pub vault_browser: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playback {
    pub default_volume: f32,
    pub shuffle: bool,
    pub repeat: bool,
    #[serde(default)]
    pub repeat_mode: Option<String>,
    #[serde(default = "default_crossfade_secs")]
    pub crossfade_secs: f32,
    #[serde(default = "default_eq_preset")]
    pub eq_preset: String,
    #[serde(default)]
    pub endless_mode: bool,
}

fn default_crossfade_secs() -> f32 {
    2.0
}

fn default_eq_preset() -> String {
    "Flat".to_string()
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
                seek_back: String::new(),
                seek_forward: String::new(),
                stop: String::new(),
                shuffle: String::new(),
                repeat: String::new(),
                search: String::new(),
                tab: String::new(),
                enqueue: String::new(),
                remove_from_queue: String::new(),
                clear_queue: String::new(),
                toggle_mini: String::new(),
                toggle_view: String::new(),
                rescan: String::new(),
                help: String::new(),
                cycle_theme: String::new(),
                open_url: String::new(),
                toggle_favorite: String::new(),
                command_palette: String::new(),
                subsonic_browser: String::new(),
                vault_browser: String::new(),
            },
            playback: Playback {
                default_volume: 0.7,
                shuffle: false,
                repeat: false,
                repeat_mode: None,
                crossfade_secs: 2.0,
                eq_preset: "Flat".to_string(),
                endless_mode: false,
            },
            spotify: SpotifyConfig::default(),
            visualizer: VisualizerConfig::default(),
            lastfm: LastfmConfig::default(),
            discord: DiscordConfig::default(),
            ytdlp: YtdlpConfig::default(),
            cache: CacheConfig::default(),
            history: HistoryConfig::default(),
            library: LibraryConfig::default(),
            subsonic: SubsonicConfig::default(),
            vault: VaultConfig::default(),
        }
    }
}

impl Config {
    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("config: failed to create dir {}: {e}", parent.display());
            }
        }
        // Always preserve credentials from disk — these are set manually by the user
        // and must never be overwritten by the runtime state saved on exit.
        let mut merged = self.clone();
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(on_disk) = toml::from_str::<Config>(&text) {
                if !on_disk.spotify.client_id.is_empty() {
                    merged.spotify.client_id = on_disk.spotify.client_id;
                }
                if !on_disk.lastfm.api_key.is_empty() {
                    merged.lastfm.api_key = on_disk.lastfm.api_key;
                    merged.lastfm.api_secret = on_disk.lastfm.api_secret;
                }
                if !on_disk.discord.client_id.is_empty() {
                    merged.discord.client_id = on_disk.discord.client_id;
                }
                if !on_disk.subsonic.server_url.is_empty() {
                    merged.subsonic = on_disk.subsonic;
                }
                if !on_disk.vault.server_url.is_empty() {
                    merged.vault = on_disk.vault;
                }
            }
        }
        atomic_write(&path, &toml::to_string_pretty(&merged)?)?;
        Ok(())
    }

    /// Load config from disk, falling back to defaults on any error. Returns
    /// accumulated warnings so the caller can surface them before the TUI
    /// takes over stderr (#97).
    pub fn load_or_default() -> Result<(Self, Vec<String>)> {
        let mut warnings: Vec<String> = Vec::new();
        let path = config_path()?;
        let cfg = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(text) => match toml::from_str::<Self>(&text) {
                    Ok(c) => c,
                    Err(e) => {
                        warnings.push(format!("config.toml: parse error — {e}; using defaults"));
                        Self::default()
                    }
                },
                Err(e) => {
                    warnings.push(format!(
                        "config.toml: could not read {} — {e}; using defaults",
                        path.display()
                    ));
                    Self::default()
                }
            }
        } else {
            let cfg = Self::default();
            if let Some(parent) = path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    warnings.push(format!(
                        "config: failed to create dir {}: {e}",
                        parent.display()
                    ));
                }
            }
            if let Err(e) = fs::write(&path, toml::to_string_pretty(&cfg).unwrap_or_default()) {
                warnings.push(format!(
                    "config: failed to write default config {}: {e}",
                    path.display()
                ));
            }
            cfg
        };
        // Validate music_dirs — warn for each that doesn't exist on disk.
        for dir in &cfg.music_dirs {
            if !dir.exists() {
                warnings.push(format!(
                    "config.toml: music_dirs entry '{}' does not exist",
                    dir.display()
                ));
            }
        }
        Ok((cfg, warnings))
    }
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "noctune", "noctune").context("could not determine config directory")
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

pub fn plugins_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("plugins"))
}

pub fn db_path() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().join("library.db"))
}

pub fn audio_cache_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.cache_dir().join("audio"))
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
    #[serde(default)]
    pub low_db: f32,
    #[serde(default)]
    pub mid_db: f32,
    #[serde(default)]
    pub high_db: f32,
    #[serde(default)]
    pub bands: Option<Vec<f32>>,
}

impl EqPreset {
    pub fn to_eq_state(&self) -> crate::eq::EqState {
        if let Some(bands) = &self.bands {
            let mut arr = [0.0f32; crate::eq::NUM_BANDS];
            for (i, &v) in bands.iter().take(crate::eq::NUM_BANDS).enumerate() {
                arr[i] = v;
            }
            crate::eq::EqState::from_bands(arr)
        } else {
            crate::eq::EqState::from_3band(self.low_db, self.mid_db, self.high_db)
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EqPresets {
    #[serde(default)]
    pub presets: Vec<EqPreset>,
}

impl EqPresets {
    pub fn load() -> Self {
        let Ok(path) = eq_presets_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match toml::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(target: "config", "eq_presets.toml: parse error — {e}; using defaults (file preserved)");
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = eq_presets_path()?;
        atomic_write(&path, &toml::to_string_pretty(self)?)?;
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
    #[serde(default)]
    pub eq_low_db: f32,
    #[serde(default)]
    pub eq_mid_db: f32,
    #[serde(default)]
    pub eq_high_db: f32,
    #[serde(default)]
    pub eq_bands: Option<Vec<f32>>,
}

impl Profile {
    pub fn to_eq_state(&self) -> crate::eq::EqState {
        if let Some(bands) = &self.eq_bands {
            let mut arr = [0.0f32; crate::eq::NUM_BANDS];
            for (i, &v) in bands.iter().take(crate::eq::NUM_BANDS).enumerate() {
                arr[i] = v;
            }
            crate::eq::EqState::from_bands(arr)
        } else {
            crate::eq::EqState::from_3band(self.eq_low_db, self.eq_mid_db, self.eq_high_db)
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profiles {
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl Profiles {
    pub fn load() -> Self {
        let Ok(path) = profiles_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match toml::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(target: "config", "profiles.toml: parse error — {e}; using defaults (file preserved)");
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = profiles_path()?;
        atomic_write(&path, &toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("config: failed to create dir {}: {e}", parent.display());
        }
    }
    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp_path, content)?;
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

fn default_music_dirs() -> Vec<PathBuf> {
    directories::UserDirs::new()
        .and_then(|u| u.audio_dir().map(|p| p.to_path_buf()))
        .map(|p| vec![p])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips_toml() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).expect("serialize default");
        let parsed: Config = toml::from_str(&text).expect("parse default");
        assert_eq!(parsed.theme, cfg.theme);
        assert_eq!(parsed.playback.default_volume, cfg.playback.default_volume);
    }

    #[test]
    fn unknown_sections_dont_break_load() {
        // serde drops unknown sections by default. This guards against a future
        // #[serde(deny_unknown_fields)] regression.
        let text = "theme = \"default\"\nmusic_dirs = []\n\n[future_section]\nfoo = 1\n";
        // Minimal toml — many required fields are missing, but parsing the unknown
        // section alone should not blow up.
        let res = toml::from_str::<Config>(text);
        // Required fields missing means err is expected; we just ensure it's not
        // a "unknown field" kind of failure.
        if let Err(e) = res {
            let msg = e.to_string();
            assert!(
                !msg.contains("unknown field `future_section`"),
                "unexpected: {msg}"
            );
        }
    }

    #[test]
    fn ytdlp_defaults_sane() {
        let d = YtdlpConfig::default();
        assert!(d.max_retries >= 1);
        assert!(d.backoff_secs >= 1);
    }

    #[test]
    fn history_defaults_sane() {
        let d = HistoryConfig::default();
        assert!(d.max_entries > 0);
    }
}
