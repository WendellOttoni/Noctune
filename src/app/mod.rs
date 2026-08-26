mod prefetch;
mod scan;
mod util;

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use image::DynamicImage;
use ratatui::widgets::ListState;
use std::{path::PathBuf, time::Duration};

use notify::Watcher as _;

// Re-exports of moved-out helpers so the rest of this module compiles unchanged.
use self::prefetch::{PrefetchSlots, PreloadedTrack, SlotKind};
use self::scan::scan_library_with_progress;
use self::util::{
    parse_spotify_url, pseudo_random, rect_contains, rg_scale, sort_tracks,
    sort_tracks_with_ratings,
};

use crate::{
    album_art::ArtPicker,
    audio::{CrossfadeStatus, Player, Track},
    cache::{cache_path, MetadataCache},
    config::Config,
    keybinds::{Action, Bindings},
    theme::Theme,
    tui::Tui,
    ui,
    visualizer::VizTap,
};

const MAX_UNDO_SNAPSHOTS: usize = 10;
const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac"];

/// Severity tag for status-bar messages (#102). Drives both the foreground
/// color in the status bar and how aggressively the message hangs around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusKind {
    #[default]
    Info,
    #[allow(dead_code)] // reserved for future amber warnings; not yet used
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Library,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Title,
    Artist,
    Album,
    Rating,
}

impl SortMode {
    pub fn cycle(self) -> Self {
        match self {
            SortMode::Title => SortMode::Artist,
            SortMode::Artist => SortMode::Album,
            SortMode::Album => SortMode::Rating,
            SortMode::Rating => SortMode::Title,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Title => "title",
            SortMode::Artist => "artist",
            SortMode::Album => "album",
            SortMode::Rating => "rating",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayGainMode {
    Off,
    Track,
    Album,
}

impl ReplayGainMode {
    pub fn cycle(self) -> Self {
        match self {
            ReplayGainMode::Off => ReplayGainMode::Track,
            ReplayGainMode::Track => ReplayGainMode::Album,
            ReplayGainMode::Album => ReplayGainMode::Off,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ReplayGainMode::Off => "off",
            ReplayGainMode::Track => "track",
            ReplayGainMode::Album => "album",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VizMode {
    Spectrum,
    Waveform,
    VuMeter,
    Waterfall,
    Oscilloscope,
}

impl VizMode {
    pub fn cycle(self) -> Self {
        match self {
            VizMode::Spectrum => VizMode::Waveform,
            VizMode::Waveform => VizMode::VuMeter,
            VizMode::VuMeter => VizMode::Waterfall,
            VizMode::Waterfall => VizMode::Oscilloscope,
            VizMode::Oscilloscope => VizMode::Spectrum,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            VizMode::Spectrum => "spectrum",
            VizMode::Waveform => "waveform",
            VizMode::VuMeter => "vu-meter",
            VizMode::Waterfall => "waterfall",
            VizMode::Oscilloscope => "oscilloscope",
        }
    }
}

#[derive(Debug, Clone)]
struct UndoSnapshot {
    queue: Vec<Track>,
    queue_index: Option<usize>,
    label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotifyTab {
    Search,
    MyPlaylists,
    LikedSongs,
}

pub struct App {
    #[allow(dead_code)]
    pub config: Config,
    pub theme: Theme,
    pub player: Player,
    pub tap: VizTap,
    pub library: Vec<Track>,
    pub queue: Vec<Track>,
    undo_stack: std::collections::VecDeque<UndoSnapshot>,
    pub library_state: ListState,
    pub queue_state: ListState,
    pub focus: Pane,
    pub queue_index: Option<usize>,
    pub status: String,
    pub status_kind: StatusKind,
    pub should_quit: bool,
    pub search: String,
    pub search_editing: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub show_help: bool,
    pub help_scroll: u16,
    pub sort: SortMode,
    pub sleep_until: Option<std::time::Instant>,
    pub history: std::collections::VecDeque<Track>,
    pub bindings: Bindings,
    pub lyrics: Option<crate::lyrics::Lyrics>,
    pub spotify_client_id: String,
    pub spotify_redirect_uri: String,
    pub spotify: Option<crate::spotify::SpotifyApi>,
    pub layout: LayoutRects,
    pub view_mode: ViewMode,
    pub pending_crossfade_idx: Option<usize>,
    pub url_input: String,
    pub url_editing: bool,
    pub eq_preset_idx: usize,
    pub show_info: bool,
    pub theme_names: Vec<String>,
    pub theme_idx: usize,
    pub last_drag_seek: Option<std::time::Instant>,
    pub clear_confirm_until: Option<std::time::Instant>,
    pub url_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Track>, String>>>,
    pub load_rx: Option<std::sync::mpsc::Receiver<Result<crate::audio::SymphoniaSource, String>>>,
    pub loading_track: Option<Track>,
    pub pending_seek_offset: Option<Duration>,
    pub prefetch: PrefetchSlots,
    pub scan_rx: Option<std::sync::mpsc::Receiver<Vec<Track>>>,
    /// (#104) Live progress events from the scan worker — `(done, total)`.
    pub scan_progress_rx: Option<std::sync::mpsc::Receiver<(usize, usize)>>,
    /// Latest progress reading consumed from `scan_progress_rx`. Cleared when
    /// the scan completes. `None` means we're idle or have not yet received a
    /// progress event.
    pub scan_progress: Option<(usize, usize)>,
    pub fs_event_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pub _fs_watcher: Option<notify::RecommendedWatcher>,
    pub theme_watcher_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pub _theme_watcher: Option<notify::RecommendedWatcher>,
    pub lyrics_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Option<crate::lyrics::Lyrics>)>>,
    /// #105: async receiver for remote album-art bytes (e.g. YouTube
    /// thumbnails). Sender is spawned from `on_track_started`; result is
    /// applied in `tick` only if the player is still on the same track.
    pub art_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Option<Vec<u8>>)>>,
    /// #86/#87: monotonic counters bumped whenever the underlying data
    /// changes. Used as the fingerprint for the cached library-view rows so
    /// the render loop skips recomputation when nothing relevant moved.
    pub library_revision: u64,
    pub history_revision: u64,
    pub play_history_revision: u64,
    /// #87: memoised result of `smart_rows()`. Recomputed only when the
    /// fingerprint (library + play history + expanded categories) changes.
    pub smart_cache: Option<SmartRowsCache>,
    /// #86: memoised result of `library_rows()` for non-Smart view modes.
    /// Rebuilt only when search / sort / view_mode / library_revision /
    /// history_revision change.
    pub library_view_cache: Option<LibraryViewCache>,
    pub radio_mode: crate::radio_mode::RadioMode,
    pub radio_fetch_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Track>, String>>>,
    pub radio_seed_editing: bool,
    pub radio_seed_input: String,
    pub show_stats: bool,
    pub show_lastfm_panel: bool,
    pub lastfm_panel_recent: Vec<crate::lastfm::RecentTrack>,
    pub lastfm_panel_top_artists: Vec<crate::lastfm::TopArtist>,
    pub lastfm_panel_rx: Option<
        std::sync::mpsc::Receiver<(
            Vec<crate::lastfm::RecentTrack>,
            Vec<crate::lastfm::TopArtist>,
        )>,
    >,
    pub media_session: Option<crate::media_session::MediaSession>,
    pub media_session_rx: Option<std::sync::mpsc::Receiver<souvlaki::MediaControlEvent>>,
    pub rescan_debounce_until: Option<std::time::Instant>,
    pub tick_count: u64,
    pub hover_x: Option<u16>,
    pub show_audio_panel: bool,
    pub audio_panel_row: usize,
    pub replaygain_mode: ReplayGainMode,
    pub viz_mode: VizMode,
    pub ratings: crate::ratings::Ratings,
    pub play_history: crate::history::PlayHistory,
    pub smart_expanded: [bool; 4],
    pub play_threshold_secs: f64,
    pub browser_path: Option<PathBuf>,
    pub browser_music_root_idx: usize,
    pub mini_mode: bool,
    current_play_recorded: bool,
    pub lastfm: Option<crate::lastfm::LastfmClient>,
    lastfm_pending_token: Option<String>,
    lastfm_scrobble_info: Option<(String, String, u64)>,
    lastfm_scrobbled: bool,
    discord_tx: Option<std::sync::mpsc::Sender<crate::discord::Cmd>>,
    pub show_device_selector: bool,
    pub device_list: Vec<String>,
    pub device_selector_row: usize,
    pub show_eq_tuner: bool,
    pub eq_tuner_band: usize,
    pending_gapless_idx: Option<usize>,
    pub playlist_name_editing: bool,
    pub playlist_name_input: String,
    pub show_playlist_browser: bool,
    pub playlist_browser_entries: Vec<PlaylistEntry>,
    pub playlist_browser_row: usize,
    pub playlist_browser_delete_confirm: Option<usize>,
    pub active_playlist_name: Option<String>,
    pub album_art: Option<DynamicImage>,
    /// Monotonic counter bumped whenever `album_art` is replaced. Used as part
    /// of the overlay cache key in `ArtPicker` (#91) so a track change
    /// invalidates the cached escape sequence without comparing image bytes.
    pub art_generation: u64,
    pub art_picker: ArtPicker,
    pub sys_stats: crate::stats::SystemStats,
    pub custom_eq_presets: Vec<crate::config::EqPreset>,
    pub eq_preset_name_editing: bool,
    pub eq_preset_name_input: String,
    pub profiles: Vec<crate::config::Profile>,
    pub show_profile_browser: bool,
    pub profile_browser_row: usize,
    pub profile_name_editing: bool,
    pub profile_name_input: String,
    pub show_spotify_browser: bool,
    pub spotify_browser_query: String,
    pub spotify_browser_query_editing: bool,
    pub spotify_browser_results: Vec<crate::audio::Track>,
    pub spotify_browser_row: usize,
    pub spotify_browser_tab: SpotifyTab,
    pub spotify_my_playlists: Vec<(String, String, u32)>,
    pub spotify_playlist_row: usize,
    spotify_search_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Track>, String>>>,
    pub show_tag_editor: bool,
    pub tag_editor_path: Option<PathBuf>,
    pub tag_editor_fields: [String; 5],
    pub tag_editor_row: usize,
    pub show_radio_browser: bool,
    pub radio_tab: crate::radio_browser::RadioTab,
    pub radio_curated_list: Vec<crate::radio_browser::RadioStation>,
    pub radio_search_results: Vec<crate::radio_browser::RadioStation>,
    pub radio_row: usize,
    pub radio_category_idx: usize,
    pub radio_focus_pane: usize,
    pub radio_search_query: String,
    pub radio_search_editing: bool,
    pub radio_search_rx: Option<std::sync::mpsc::Receiver<Result<Vec<crate::radio_browser::RadioStation>, String>>>,
    pub update_info: Option<crate::updater::UpdateInfo>,
    pub is_updating: bool,
    update_check_rx: Option<std::sync::mpsc::Receiver<Result<Option<crate::updater::UpdateInfo>, String>>>,
    update_apply_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
}

#[derive(Debug, Clone)]
pub struct PlaylistEntry {
    pub name: String,
    pub path: std::path::PathBuf,
    pub track_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutRects {
    pub library: ratatui::layout::Rect,
    pub queue: ratatui::layout::Rect,
    pub progress: ratatui::layout::Rect,
    pub progress_total_ms: u64,
    pub art_area: ratatui::layout::Rect,
}

#[derive(Debug, Clone)]
pub enum LibraryRow {
    Header(String),
    SmartHeader {
        label: String,
        count: usize,
        expanded: bool,
    },
    /// #92: tracks are reference-counted so the cached `Vec<LibraryRow>` can
    /// be cheaply cloned per frame (refcount bump instead of deep-cloning the
    /// `Track`'s owned strings).
    Track(std::sync::Arc<Track>),
    Dir(std::path::PathBuf),
}

/// #87: cached Smart-view rows + the fingerprint of inputs that produced them.
/// Stored on `App` and reused across frames while the fingerprint matches.
#[derive(Debug)]
pub struct SmartRowsCache {
    pub library_revision: u64,
    pub history_revision: u64,
    pub play_history_revision: u64,
    pub expanded: [bool; 4],
    pub rows: Vec<LibraryRow>,
}

/// #86: fingerprint of every input that can change the non-Smart library view
/// rows (Flat / Albums / RecentlyPlayed). When this matches the cached value,
/// the render loop skips re-filtering and re-cloning the entire library.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryViewFingerprint {
    library_revision: u64,
    history_revision: u64,
    view_mode: ViewMode,
    sort: SortMode,
    search: String,
}

#[derive(Debug)]
pub struct LibraryViewCache {
    fingerprint: LibraryViewFingerprint,
    rows: Vec<LibraryRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Flat,
    Albums,
    RecentlyPlayed,
    Smart,
    Browser,
    Radio,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            ViewMode::Flat => ViewMode::Albums,
            ViewMode::Albums => ViewMode::Smart,
            ViewMode::Smart => ViewMode::Browser,
            ViewMode::Browser => ViewMode::Radio,
            ViewMode::Radio => ViewMode::Flat,
            ViewMode::RecentlyPlayed => ViewMode::Flat,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Flat => "flat",
            ViewMode::Albums => "albums",
            ViewMode::RecentlyPlayed => "recently played",
            ViewMode::Smart => "smart",
            ViewMode::Browser => "browser",
            ViewMode::Radio => "radio",
        }
    }
}

impl App {
    /// Replace the status bar text and update its severity (#102). Always
    /// prefer this over poking `self.status` directly so the color in the
    /// status bar stays in sync with the message intent.
    pub fn set_status<S: Into<String>>(&mut self, kind: StatusKind, msg: S) {
        // Write the fields directly here. Calling `set_info` from this method
        // (as an earlier sed-driven refactor accidentally did) creates mutual
        // recursion with `set_info` → stack overflow on the first status change.
        self.status = msg.into();
        self.status_kind = kind;
    }

    /// Convenience for the common Info case.
    pub fn set_info<S: Into<String>>(&mut self, msg: S) {
        self.set_status(StatusKind::Info, msg);
    }

    /// Convenience for the common Error case.
    pub fn set_error<S: Into<String>>(&mut self, msg: S) {
        self.set_status(StatusKind::Error, msg);
    }

    pub fn new(config: Config, theme: Theme, art_picker: ArtPicker) -> Result<Self> {
        crate::ytdlp::configure_retries(config.ytdlp.clone());
        let history_cfg = config.history.clone();
        let player = Player::new(
            config.playback.default_volume,
            config.visualizer.sensitivity,
        )?;
        let tap = player.tap();

        let config_shuffle = config.playback.shuffle;
        let config_repeat = config.playback.repeat;
        let config_keybinds = config.keybinds.clone();
        let spotify_client_id = config.spotify.client_id.clone();
        let spotify_redirect_uri = config.spotify.redirect_uri();
        let spotify_port = config.spotify.redirect_port;

        let discord_tx = if config.discord.is_configured() {
            Some(crate::discord::spawn(config.discord.client_id.clone()))
        } else {
            None
        };

        let lastfm = if config.lastfm.is_configured() {
            crate::lastfm::load_session().and_then(|s| {
                crate::lastfm::LastfmClient::new(
                    config.lastfm.api_key.clone(),
                    config.lastfm.api_secret.clone(),
                    s,
                )
                .ok()
            })
        } else {
            None
        };
        let _ = spotify_port;

        let spotify = crate::spotify::load_tokens()
            .filter(|_| !spotify_client_id.is_empty())
            .and_then(|t| crate::spotify::SpotifyApi::new(spotify_client_id.clone(), t).ok());

        // Start async library scan
        let (scan_tx, scan_rx) = std::sync::mpsc::channel::<Vec<Track>>();
        let (progress_tx, scan_progress_rx) = std::sync::mpsc::channel::<(usize, usize)>();
        let scan_dirs = config.music_dirs.clone();
        let cache_cfg = config.cache.clone();
        std::thread::spawn(move || {
            let cache_file = cache_path();
            let mut cache = cache_file
                .as_ref()
                .map(|p| MetadataCache::load(p))
                .unwrap_or_default();
            // #70: drop stale entries before re-scanning so removed files do not stay
            // in the cache forever.
            cache.prune(cache_cfg.expire_days, cache_cfg.max_size_mb);
            let tracks = scan_library_with_progress(&scan_dirs, &mut cache, Some(progress_tx));
            if let Some(p) = &cache_file {
                cache.save(p);
            }
            let _ = scan_tx.send(tracks);
        });

        // Start filesystem watcher — opt-out via [library].watch_for_changes (#72).
        let (fs_tx, fs_event_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut _fs_watcher = if config.library.watch_for_changes {
            let w = match notify::RecommendedWatcher::new(fs_tx, notify::Config::default()) {
                Ok(w) => Some(w),
                Err(e) => {
                    tracing::warn!(target: "library", "fs watcher: failed to init: {e}");
                    None
                }
            };
            if let Some(mut watcher) = w {
                for dir in &config.music_dirs {
                    if let Err(e) = watcher.watch(dir.as_path(), notify::RecursiveMode::Recursive) {
                        tracing::warn!(target: "library", "fs watcher: failed to watch {}: {e}", dir.display());
                    }
                }
                Some(watcher)
            } else {
                None
            }
        } else {
            None
        };

        let mut app = Self {
            config,
            theme,
            player,
            tap,
            library: Vec::new(),
            queue: Vec::new(),
            undo_stack: std::collections::VecDeque::with_capacity(MAX_UNDO_SNAPSHOTS),
            library_state: ListState::default(),
            queue_state: ListState::default(),
            focus: Pane::Library,
            queue_index: None,
            status: "Scanning library…".into(),
            status_kind: StatusKind::Info,
            should_quit: false,
            search: String::new(),
            search_editing: false,
            shuffle: config_shuffle,
            repeat: if config_repeat {
                RepeatMode::All
            } else {
                RepeatMode::Off
            },
            show_help: false,
            help_scroll: 0,
            sort: SortMode::Title,
            sleep_until: None,
            history: std::collections::VecDeque::with_capacity(64),
            bindings: {
                let (b, warnings) = Bindings::from_config(&config_keybinds);
                for w in warnings {
                    tracing::warn!(target: "keybinds", "{w}");
                }
                b
            },
            lyrics: None,
            spotify_client_id,
            spotify_redirect_uri,
            spotify,
            layout: LayoutRects::default(),
            view_mode: ViewMode::Flat,
            pending_crossfade_idx: None,
            url_input: String::new(),
            url_editing: false,
            eq_preset_idx: 0,
            show_info: false,
            theme_names: Vec::new(),
            theme_idx: 0,
            last_drag_seek: None,
            clear_confirm_until: None,
            url_rx: None,
            load_rx: None,
            loading_track: None,
            pending_seek_offset: None,
            prefetch: PrefetchSlots::new(),
            scan_rx: Some(scan_rx),
            scan_progress_rx: Some(scan_progress_rx),
            scan_progress: None,
            fs_event_rx: Some(fs_event_rx),
            _fs_watcher,
            theme_watcher_rx: None,
            _theme_watcher: None,
            lyrics_rx: None,
            art_rx: None,
            library_revision: 0,
            history_revision: 0,
            play_history_revision: 0,
            smart_cache: None,
            library_view_cache: None,
            radio_mode: crate::radio_mode::RadioMode::default(),
            radio_fetch_rx: None,
            radio_seed_editing: false,
            radio_seed_input: String::new(),
            show_stats: false,
            show_lastfm_panel: false,
            lastfm_panel_recent: Vec::new(),
            lastfm_panel_top_artists: Vec::new(),
            lastfm_panel_rx: None,
            media_session: None,
            media_session_rx: None,
            rescan_debounce_until: None,
            tick_count: 0,
            hover_x: None,
            show_audio_panel: false,
            audio_panel_row: 0,
            replaygain_mode: ReplayGainMode::Track,
            viz_mode: VizMode::Spectrum,
            ratings: crate::ratings::Ratings::load(),
            play_history: {
                let mut h = crate::history::PlayHistory::load();
                // #70: enforce history retention at startup so it does not grow forever.
                h.prune(history_cfg.max_entries, history_cfg.retain_days);
                h
            },
            smart_expanded: [true, false, false, false],
            play_threshold_secs: 30.0,
            current_play_recorded: false,
            browser_path: None,
            browser_music_root_idx: 0,
            mini_mode: false,
            playlist_name_editing: false,
            playlist_name_input: String::new(),
            show_playlist_browser: false,
            playlist_browser_entries: Vec::new(),
            playlist_browser_row: 0,
            playlist_browser_delete_confirm: None,
            active_playlist_name: None,
            lastfm,
            lastfm_pending_token: None,
            lastfm_scrobble_info: None,
            lastfm_scrobbled: false,
            discord_tx,
            show_device_selector: false,
            device_list: Vec::new(),
            device_selector_row: 0,
            show_eq_tuner: false,
            eq_tuner_band: 0,
            pending_gapless_idx: None,
            album_art: None,
            art_generation: 0,
            art_picker,
            sys_stats: crate::stats::SystemStats::new(),
            custom_eq_presets: crate::config::EqPresets::load().presets,
            eq_preset_name_editing: false,
            eq_preset_name_input: String::new(),
            profiles: crate::config::Profiles::load().profiles,
            show_profile_browser: false,
            profile_browser_row: 0,
            profile_name_editing: false,
            profile_name_input: String::new(),
            show_spotify_browser: false,
            spotify_browser_query: String::new(),
            spotify_browser_query_editing: false,
            spotify_browser_results: Vec::new(),
            spotify_browser_row: 0,
            spotify_browser_tab: SpotifyTab::Search,
            spotify_my_playlists: Vec::new(),
            spotify_playlist_row: 0,
            spotify_search_rx: None,
            show_tag_editor: false,
            tag_editor_path: None,
            tag_editor_fields: Default::default(),
            tag_editor_row: 0,
            show_radio_browser: false,
            radio_tab: crate::radio_browser::RadioTab::Curated,
            radio_curated_list: crate::radio_browser::curated_stations(),
            radio_search_results: Vec::new(),
            radio_row: 0,
            radio_category_idx: 0,
            radio_focus_pane: 0,
            radio_search_query: String::new(),
            radio_search_editing: false,
            radio_search_rx: None,
            update_info: None,
            is_updating: false,
            update_check_rx: None,
            update_apply_rx: None,
        };

        // Spawn background update check on startup
        let (update_tx, update_rx) = std::sync::mpsc::channel();
        app.update_check_rx = Some(update_rx);
        std::thread::spawn(move || {
            let res = crate::updater::check_for_updates().map_err(|e| e.to_string());
            let _ = update_tx.send(res);
        });

        // Watch the active theme file so external edits hot-reload (#68).
        app.rearm_theme_watcher();
        // OS media session (#54) — disabled if souvlaki cannot create the controls
        // (e.g. headless Linux without a dbus session). Log the failure so users can
        // tell apart "no SMTC card because the integration is off" from "no SMTC card
        // because something else is wrong".
        match crate::media_session::MediaSession::new("Noctune") {
            Ok((session, rx)) => {
                app.media_session = Some(session);
                app.media_session_rx = Some(rx);
                tracing::info!(target: "media_session", "OS media session active");
            }
            Err(e) => {
                tracing::warn!(target: "media_session", "could not init OS media session: {e}");
            }
        }
        Ok(app)
    }

    pub fn is_loading(&self) -> bool {
        self.url_rx.is_some() || self.scan_rx.is_some() || self.spotify_search_rx.is_some()
    }

    pub fn search_active(&self) -> bool {
        self.search_editing
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    pub fn visible_library(&self) -> Vec<&Track> {
        let source: Box<dyn Iterator<Item = &Track>> = match self.view_mode {
            ViewMode::RecentlyPlayed => Box::new(self.history.iter()),
            _ => Box::new(self.library.iter()),
        };

        if self.search.is_empty() {
            source.collect()
        } else {
            let needle = self.search.to_lowercase();
            source
                .filter(|t| {
                    let matches = |s: &str| s.to_lowercase().contains(&needle);
                    matches(&t.title)
                        || t.artist.as_deref().map(matches).unwrap_or(false)
                        || t.album.as_deref().map(matches).unwrap_or(false)
                        || t.genre.as_deref().map(matches).unwrap_or(false)
                        || t.year.as_deref().map(matches).unwrap_or(false)
                })
                .collect()
        }
    }

    pub fn library_rows(&mut self) -> Vec<LibraryRow> {
        if self.view_mode == ViewMode::Smart {
            return self.smart_rows_cached().to_vec();
        }
        if self.view_mode == ViewMode::Browser {
            // Filesystem-backed and infrequent; not worth caching against an
            // external mtime fingerprint here.
            return self.browser_rows();
        }
        self.library_rows_cached().to_vec()
    }

    /// #86: cached non-Smart library rows. Rebuild only when the fingerprint
    /// changes; the render loop otherwise reuses the existing Vec without
    /// re-running `to_lowercase` across the whole library on every frame.
    fn library_rows_cached(&mut self) -> &[LibraryRow] {
        let fp = LibraryViewFingerprint {
            library_revision: self.library_revision,
            history_revision: self.history_revision,
            view_mode: self.view_mode,
            sort: self.sort,
            search: self.search.clone(),
        };
        let stale = match &self.library_view_cache {
            Some(c) => c.fingerprint != fp,
            None => true,
        };
        if stale {
            let rows = self.build_library_rows();
            self.library_view_cache = Some(LibraryViewCache {
                fingerprint: fp,
                rows,
            });
        }
        self.library_view_cache
            .as_ref()
            .map(|c| c.rows.as_slice())
            .unwrap_or(&[])
    }

    fn build_library_rows(&self) -> Vec<LibraryRow> {
        let visible = self.visible_library();
        if self.view_mode == ViewMode::Flat
            || self.view_mode == ViewMode::RecentlyPlayed
            || self.sort != SortMode::Album
        {
            return visible
                .into_iter()
                .map(|t| LibraryRow::Track(std::sync::Arc::new(t.clone())))
                .collect();
        }
        let mut out = Vec::with_capacity(visible.len() + 16);
        let mut last_album: Option<String> = None;
        for t in visible {
            let album = t.album.clone().unwrap_or_else(|| "Unknown Album".into());
            if last_album.as_deref() != Some(album.as_str()) {
                out.push(LibraryRow::Header(album.clone()));
                last_album = Some(album);
            }
            out.push(LibraryRow::Track(std::sync::Arc::new(t.clone())));
        }
        out
    }

    /// Returns the cached Smart-view rows, rebuilding them only when the
    /// fingerprint (library / history / play-history revisions, expanded
    /// flags) has changed since the last call (#87). The previous version
    /// rebuilt all four categories — and ran `fs::metadata` per library track
    /// for "Recently Added" — every frame.
    fn smart_rows_cached(&mut self) -> &[LibraryRow] {
        let stale = match &self.smart_cache {
            Some(c) => {
                c.library_revision != self.library_revision
                    || c.history_revision != self.history_revision
                    || c.play_history_revision != self.play_history_revision
                    || c.expanded != self.smart_expanded
            }
            None => true,
        };
        if stale {
            let rows = self.build_smart_rows();
            self.smart_cache = Some(SmartRowsCache {
                library_revision: self.library_revision,
                history_revision: self.history_revision,
                play_history_revision: self.play_history_revision,
                expanded: self.smart_expanded,
                rows,
            });
        }
        // Just refreshed (or already fresh) above.
        self.smart_cache
            .as_ref()
            .map(|c| c.rows.as_slice())
            .unwrap_or(&[])
    }

    fn build_smart_rows(&self) -> Vec<LibraryRow> {
        const LIMIT: usize = 50;
        let track_map: std::collections::HashMap<String, &Track> = self
            .library
            .iter()
            .map(|t| (t.path.display().to_string(), t))
            .collect();

        let most_played: Vec<Track> = {
            let paths = self.play_history.most_played_paths(LIMIT);
            paths
                .iter()
                .filter_map(|(k, _)| track_map.get(k).copied().cloned())
                .collect()
        };

        let recently_played: Vec<Track> = {
            let paths = self.play_history.recently_played_paths(LIMIT);
            paths
                .iter()
                .filter_map(|k| track_map.get(k).copied().cloned())
                .collect()
        };

        let never_played: Vec<Track> = {
            let mut v: Vec<Track> = self
                .library
                .iter()
                .filter(|t| self.play_history.play_count(&t.path) == 0)
                .cloned()
                .collect();
            v.truncate(LIMIT);
            v
        };

        // #87: use the `added_at` field populated at scan time instead of
        // calling `fs::metadata` per track on every frame.
        let recently_added: Vec<Track> = {
            let mut idx: Vec<(usize, u64)> = self
                .library
                .iter()
                .enumerate()
                .map(|(i, t)| (i, t.added_at.unwrap_or(0)))
                .collect();
            idx.sort_by(|a, b| b.1.cmp(&a.1));
            idx.truncate(LIMIT);
            idx.into_iter()
                .map(|(i, _)| self.library[i].clone())
                .collect()
        };

        let categories: [(&str, &Vec<Track>, bool); 4] = [
            ("Most Played", &most_played, self.smart_expanded[0]),
            ("Recently Played", &recently_played, self.smart_expanded[1]),
            ("Recently Added", &recently_added, self.smart_expanded[2]),
            ("Never Played", &never_played, self.smart_expanded[3]),
        ];

        let mut out = Vec::new();
        for (label, tracks, expanded) in categories {
            out.push(LibraryRow::SmartHeader {
                label: label.to_string(),
                count: tracks.len(),
                expanded,
            });
            if expanded {
                for t in tracks.iter() {
                    out.push(LibraryRow::Track(std::sync::Arc::new(t.clone())));
                }
            }
        }
        out
    }

    fn browser_rows(&self) -> Vec<LibraryRow> {
        let dir = if let Some(p) = &self.browser_path {
            p.clone()
        } else if let Some(root) = self.config.music_dirs.get(self.browser_music_root_idx) {
            root.clone()
        } else {
            return Vec::new();
        };

        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut files: Vec<Track> = Vec::new();

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if AUDIO_EXTS.contains(&ext.to_lowercase().as_str()) {
                    files.push(Track::from_path(path));
                }
            }
        }
        dirs.sort();
        files.sort_by_key(|a| a.title.to_lowercase());

        let mut out = Vec::new();
        for d in dirs {
            out.push(LibraryRow::Dir(d));
        }
        for f in files {
            out.push(LibraryRow::Track(std::sync::Arc::new(f)));
        }
        out
    }

    pub fn browser_current_path(&self) -> PathBuf {
        if let Some(p) = &self.browser_path {
            p.clone()
        } else {
            self.config
                .music_dirs
                .get(self.browser_music_root_idx)
                .cloned()
                .unwrap_or_default()
        }
    }

    fn browser_enter(&mut self) {
        let sel = self.library_state.selected().unwrap_or(0);
        let rows = self.library_rows();
        match rows.get(sel) {
            Some(LibraryRow::Dir(p)) => {
                self.browser_path = Some(p.clone());
                self.library_state.select(Some(0));
            }
            Some(LibraryRow::Track(arc)) => {
                let t = (**arc).clone();
                self.queue.push(t);
                let idx = self.queue.len() - 1;
                self.queue_index = Some(idx);
                self.queue_state.select(Some(idx));
                self.play_current();
            }
            _ => {}
        }
    }

    fn browser_up(&mut self) {
        if let Some(current) = &self.browser_path {
            let parent = current.parent().map(|p| p.to_path_buf());
            let is_root = self
                .config
                .music_dirs
                .get(self.browser_music_root_idx)
                .map(|r| current == r)
                .unwrap_or(false);
            if is_root {
                self.browser_path = None;
            } else {
                self.browser_path = parent;
            }
            self.library_state.select(Some(0));
        } else if self.config.music_dirs.len() > 1 {
            self.browser_music_root_idx =
                (self.browser_music_root_idx + 1) % self.config.music_dirs.len();
            self.library_state.select(Some(0));
        }
    }

    fn selected_library_track(&mut self) -> Option<Track> {
        let rows = self.library_rows();
        let idx = self.library_state.selected()?;
        match rows.get(idx)? {
            LibraryRow::Track(arc) => Some((**arc).clone()),
            LibraryRow::Header(_) | LibraryRow::SmartHeader { .. } | LibraryRow::Dir(_) => None,
        }
    }

    fn toggle_smart_category(&mut self, row_idx: usize) {
        let rows = self.library_rows();
        let mut cat_idx = 0usize;
        for (i, row) in rows.iter().enumerate() {
            if let LibraryRow::SmartHeader { .. } = row {
                if i == row_idx {
                    self.smart_expanded[cat_idx] = !self.smart_expanded[cat_idx];
                    return;
                }
                cat_idx += 1;
            }
        }
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| ui::render(f, self))?;
            self.render_overlay_art();
            self.tick()?;
            if event::poll(Duration::from_millis(33))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
                    Event::Mouse(m) => self.on_mouse(m),
                    _ => {}
                }
            }
        }
        // Persist session state so the next launch starts with the same settings.
        self.config.playback.default_volume = self.player.volume();
        self.config.playback.shuffle = self.shuffle;
        self.config.playback.repeat = matches!(self.repeat, RepeatMode::All | RepeatMode::One);
        let _ = self.config.save();
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        self.tick_count = self.tick_count.wrapping_add(1);

        // #54 follow-up: pump Win32 messages so SMTC callbacks reach us. Without
        // this the OS may not surface our media card at all. No-op on non-Windows.
        crate::media_session::pump_messages();

        // Refresh system stats once per second (~30 ticks at 33ms each)
        if self.tick_count.is_multiple_of(30) {
            self.sys_stats.refresh();
        }

        // (#104) Drain any pending scan progress events into the latest reading
        // before polling the completion channel. Keeping the latest only — the
        // status bar doesn't care about intermediate states.
        if let Some(rx) = &self.scan_progress_rx {
            while let Ok(p) = rx.try_recv() {
                self.scan_progress = Some(p);
            }
        }

        // Poll completed library scan
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(mut tracks) => {
                    sort_tracks(&mut tracks, self.sort);
                    let prev_n = self.library.len();
                    let n = tracks.len();
                    // #72: drop queued tracks whose source file disappeared so the
                    // queue doesn't accumulate dead entries when the user moves files
                    // around while the app is open.
                    let live_paths: std::collections::HashSet<PathBuf> =
                        tracks.iter().map(|t| t.path.clone()).collect();
                    let before_queue = self.queue.len();
                    self.queue.retain(|t| {
                        let p = t.path.to_string_lossy();
                        p.starts_with("http://")
                            || p.starts_with("https://")
                            || p.starts_with("spotify:")
                            || live_paths.contains(&t.path)
                    });
                    let removed_from_queue = before_queue - self.queue.len();
                    self.library = tracks;
                    self.library_revision = self.library_revision.wrapping_add(1);
                    self.library_state.select(if self.library.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                    self.set_info(match (n as i64 - prev_n as i64, removed_from_queue) {
                        (0, 0) if prev_n > 0 => format!("Library: {n} tracks (unchanged)."),
                        (d, 0) if d > 0 => format!("Library: +{d} → {n} tracks."),
                        (d, 0) if d < 0 => format!("Library: {d} → {n} tracks."),
                        (_, r) if r > 0 => format!("Library: {n} tracks ({r} dropped from queue)."),
                        _ => format!("Library: {n} tracks."),
                    });
                    self.scan_rx = None;
                    self.scan_progress_rx = None;
                    self.scan_progress = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.set_error(
                        "Library: scan failed — check permissions on music_dirs in config.toml",
                    );
                    self.scan_rx = None;
                    self.scan_progress_rx = None;
                    self.scan_progress = None;
                }
            }
        }

        // Drain filesystem events and set debounce timer
        if let Some(rx) = &self.fs_event_rx {
            loop {
                match rx.try_recv() {
                    Ok(Ok(event)) => {
                        let relevant = matches!(
                            event.kind,
                            notify::EventKind::Create(_)
                                | notify::EventKind::Remove(_)
                                | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
                        );
                        if relevant {
                            // #72: debounce window comes from config so users can tune
                            // for slow disks or large albums dropped in at once.
                            self.rescan_debounce_until = Some(
                                std::time::Instant::now()
                                    + Duration::from_millis(self.config.library.watch_debounce_ms),
                            );
                        }
                    }
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }

        // Theme hot-reload — pick up external edits to the active theme file (#68).
        if let Some(rx) = &self.theme_watcher_rx {
            let mut reload = false;
            loop {
                match rx.try_recv() {
                    Ok(Ok(_)) => {
                        reload = true;
                    }
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
            if reload {
                let name = self.theme.name.clone();
                if let Ok(t) = crate::theme::Theme::load(&name) {
                    self.theme = t;
                    self.set_info(format!("Theme reloaded: {name}"));
                }
            }
        }

        // Trigger debounced rescan
        if let Some(until) = self.rescan_debounce_until {
            if std::time::Instant::now() >= until && self.scan_rx.is_none() {
                self.rescan_debounce_until = None;
                self.start_async_scan();
                self.set_info("Library changed — rescanning…");
            }
        }

        if let Some(rx) = &self.spotify_search_rx {
            match rx.try_recv() {
                Ok(Ok(tracks)) => {
                    let n = tracks.len();
                    self.spotify_browser_results = tracks;
                    self.spotify_browser_row = 0;
                    self.set_info(format!("Spotify: {n} results."));
                    self.spotify_search_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Spotify: search failed — {e}"));
                    self.spotify_search_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.spotify_search_rx = None;
                }
            }
        }

        if let Some(rx) = &self.url_rx {
            match rx.try_recv() {
                Ok(Ok(tracks)) => {
                    let n = tracks.len();
                    let was_empty = self.queue.is_empty();
                    self.queue.extend(tracks);
                    if was_empty {
                        self.queue_state.select(Some(0));
                    }
                    self.set_info(format!("Added {n} track(s) to queue."));
                    self.url_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Playlist: load failed — {e}"));
                    self.url_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.set_error(
                        "yt-dlp: worker disconnected — check yt-dlp install (see README)",
                    );
                    self.url_rx = None;
                }
            }
        }
        // Poll the background track loader (#58)
        if let Some(rx) = &self.load_rx {
            match rx.try_recv() {
                Ok(Ok(source)) => {
                    let track = self.loading_track.take();
                    let seek_offset = self.pending_seek_offset.take();
                    self.load_rx = None;
                    if let Some(t) = track {
                        let offset = seek_offset.unwrap_or(Duration::ZERO);
                        match self.player.play_prepared(source, &t, offset) {
                            Ok(_) => {
                                if seek_offset.is_some() {
                                    self.set_info(format!("Playing: {}", t.display()));
                                } else {
                                    self.on_track_started(t);
                                }
                            }
                            Err(e) => self.set_error(format!("Playback: {e}")),
                        }
                    }
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Playlist: load failed — {e}"));
                    self.load_rx = None;
                    self.loading_track = None;
                    self.pending_seek_offset = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.load_rx = None;
                    self.loading_track = None;
                    self.pending_seek_offset = None;
                }
            }
        }

        // Poll prefetch channel for adjacent track pre-buffering
        if let Some(rx) = &self.prefetch.rx {
            while let Ok((kind, path, res)) = rx.try_recv() {
                match kind {
                    SlotKind::Next => {
                        self.prefetch.building_next = None;
                        if let Ok(source) = res {
                            let cur = self.queue_index.unwrap_or(0);
                            let expected_next = self.pick_next_index(cur).and_then(|i| self.queue.get(i));
                            if expected_next.map(|t| &t.path) == Some(&path) {
                                self.prefetch.next = Some(PreloadedTrack { path, source });
                            }
                        }
                    }
                    SlotKind::Prev => {
                        self.prefetch.building_prev = None;
                        if let Ok(source) = res {
                            let cur = self.queue_index.unwrap_or(0);
                            let expected_prev_idx = if cur == 0 {
                                if self.queue.len() > 1 { Some(self.queue.len() - 1) } else { None }
                            } else {
                                Some(cur - 1)
                            };
                            let expected_prev = expected_prev_idx.and_then(|i| self.queue.get(i));
                            if expected_prev.map(|t| &t.path) == Some(&path) {
                                self.prefetch.prev = Some(PreloadedTrack { path, source });
                            }
                        }
                    }
                }
            }
        }

        // Poll radio search results
        if let Some(rx) = &self.radio_search_rx {
            match rx.try_recv() {
                Ok(Ok(stations)) => {
                    let n = stations.len();
                    self.radio_search_results = stations;
                    self.radio_row = 0;
                    self.set_info(format!("Radio: found {n} stations."));
                    self.radio_search_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Radio search error: {e}"));
                    self.radio_search_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.radio_search_rx = None;
                }
            }
        }

        // Poll background update checks
        if let Some(rx) = &self.update_check_rx {
            match rx.try_recv() {
                Ok(Ok(Some(info))) => {
                    self.set_info(format!("✨ Update v{} available! Press Shift+U to update.", info.latest_version));
                    self.update_info = Some(info);
                    self.update_check_rx = None;
                }
                Ok(Ok(None)) => {
                    self.update_check_rx = None;
                }
                Ok(Err(e)) => {
                    tracing::debug!(target: "updater", "update check error: {e}");
                    self.update_check_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.update_check_rx = None;
                }
            }
        }

        // Poll binary replacement result
        if let Some(rx) = &self.update_apply_rx {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    self.is_updating = false;
                    self.set_info("✅ Noctune updated successfully! Restart the app to apply.");
                    self.update_info = None;
                    self.update_apply_rx = None;
                }
                Ok(Err(e)) => {
                    self.is_updating = false;
                    self.set_error(format!("Update failed: {e}"));
                    self.update_apply_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_updating = false;
                    self.update_apply_rx = None;
                }
            }
        }

        if let Some(err) = self.player.take_stream_error() {
            self.set_info(err);
        }

        if let Some(title) = self.player.take_stream_title() {
            let (artist, song) = if let Some((a, s)) = title.split_once(" - ") {
                (Some(a.trim().to_string()), s.trim().to_string())
            } else {
                (None, title.clone())
            };

            if let Some(current) = self.player.current_mut() {
                if let Some(a) = &artist {
                    current.artist = Some(a.clone());
                }
                current.title = song.clone();
            }

            if let Some(idx) = self.queue_index {
                if let Some(track) = self.queue.get_mut(idx) {
                    if let Some(a) = &artist {
                        track.artist = Some(a.clone());
                    }
                    track.title = song.clone();
                }
            }

            if let Some(current) = self.player.current() {
                if let Some(s) = &mut self.media_session {
                    s.update_metadata(
                        &current.title,
                        current.artist.as_deref().unwrap_or(""),
                        current.album.as_deref(),
                        current.duration,
                    );
                }
                self.set_info(format!("Radio: {}", current.display()));
            }
        }

        // OS media-session events (#54) — play/pause/next/prev pressed on the SMTC,
        // MPRIS, or MediaRemote card. Drain everything queued this tick.
        if let Some(rx) = &self.media_session_rx {
            let mut events = Vec::new();
            let mut disconnected = false;
            loop {
                match rx.try_recv() {
                    Ok(ev) => events.push(ev),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            for ev in events {
                self.handle_media_event(ev);
            }
            if disconnected {
                self.media_session_rx = None;
            }
        }
        // Keep the OS card progress in sync with playback. Cheap (just sets a field).
        if let Some(s) = &mut self.media_session {
            if self.player.current().is_some() {
                s.update_playback(!self.player.is_paused(), self.player.elapsed());
            }
        }

        // Last.fm panel async load (#63)
        if let Some(rx) = &self.lastfm_panel_rx {
            match rx.try_recv() {
                Ok((recent, top)) => {
                    self.lastfm_panel_recent = recent;
                    self.lastfm_panel_top_artists = top;
                    self.lastfm_panel_rx = None;
                    self.set_info("Last.fm: ready.");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.lastfm_panel_rx = None;
                }
            }
        }

        // LRCLIB async result (#62)
        if let Some(rx) = &self.lyrics_rx {
            match rx.try_recv() {
                Ok((track_path, Some(lyrics))) => {
                    // Only apply if the user did not move on to another track meanwhile.
                    if self
                        .player
                        .current()
                        .map(|c| c.path == track_path)
                        .unwrap_or(false)
                    {
                        self.lyrics = Some(lyrics);
                    }
                    self.lyrics_rx = None;
                }
                Ok((_, None)) => {
                    self.lyrics_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.lyrics_rx = None;
                }
            }
        }

        // #105: remote album-art async result.
        if let Some(rx) = &self.art_rx {
            match rx.try_recv() {
                Ok((track_path, Some(bytes))) => {
                    if self
                        .player
                        .current()
                        .map(|c| c.path == track_path)
                        .unwrap_or(false)
                    {
                        if let Some(img) = self.art_picker.load(&bytes) {
                            self.album_art = Some(img);
                            self.art_generation = self.art_generation.wrapping_add(1);
                            self.art_picker.invalidate();
                        }
                    }
                    self.art_rx = None;
                }
                Ok((_, None)) => {
                    self.art_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.art_rx = None;
                }
            }
        }
        if let Some(when) = self.sleep_until {
            if std::time::Instant::now() >= when {
                self.player.stop();
                self.sleep_until = None;
                self.set_info("Sleep timer reached — playback stopped.");
                return Ok(());
            }
        }

        // Advance active crossfade each tick
        if self.player.is_crossfading() {
            if let CrossfadeStatus::Complete = self.player.update_crossfade() {
                if let Some(idx) = self.pending_crossfade_idx.take() {
                    self.queue_index = Some(idx);
                    self.queue_state.select(Some(idx));
                    if let Some(t) = self.player.current().cloned() {
                        self.lyrics = crate::lyrics::Lyrics::for_track(&t.path);
                        self.set_info(format!("Playing: {}", t.display()));
                        self.push_history(t);
                    }
                }
            }
            return Ok(());
        }

        // Try to start a crossfade when close to end (not for repeat:one)
        if !matches!(self.repeat, RepeatMode::One) {
            if let Some(remaining) = self.player.remaining() {
                let xfade = Duration::from_secs_f32(self.player.crossfade_secs);
                if remaining > Duration::ZERO && remaining <= xfade {
                    let cur = self.queue_index.unwrap_or(0);
                    if let Some(next_idx) = self.pick_next_index(cur) {
                        if let Some(track) = self.queue.get(next_idx).cloned() {
                            if self.player.begin_crossfade(&track).is_ok() {
                                self.pending_crossfade_idx = Some(next_idx);
                            }
                        }
                    }
                }
            }
        }

        // Gapless: detect when queued track becomes active (sink_queue_len drops to 1)
        if self.player.gapless_queued.is_some() && self.player.sink_queue_len() <= 1 {
            if let Some(next_track) = self.player.gapless_queued.take() {
                if let Some(idx) = self.pending_gapless_idx.take() {
                    self.queue_index = Some(idx);
                    self.queue_state.select(Some(idx));
                }
                self.lyrics = crate::lyrics::Lyrics::for_track(&next_track.path);
                self.set_info(format!("Playing: {}", next_track.display()));
                self.current_play_recorded = false;
                let artist = next_track.artist.clone().unwrap_or_default();
                let title = next_track.title.clone();
                let ts = crate::lastfm::now_unix();
                self.lastfm_scrobble_info = Some((artist.clone(), title.clone(), ts));
                self.lastfm_scrobbled = false;
                if let Some(lfm) = self.lastfm.clone() {
                    let a = artist.clone();
                    let ti = title.clone();
                    std::thread::spawn(move || {
                        let _ = lfm.update_now_playing(&a, &ti);
                    });
                }
                if let Some(tx) = &self.discord_tx {
                    let _ = tx.send(crate::discord::Cmd::Update {
                        title,
                        artist,
                        start_secs: ts as i64,
                    });
                }
                self.push_history(next_track);
            }
        }

        // Gapless: pre-enqueue next track when approaching end (only when crossfade is off)
        if self.player.crossfade_secs == 0.0
            && self.player.gapless_queued.is_none()
            && self.pending_crossfade_idx.is_none()
            && !matches!(self.repeat, RepeatMode::One)
        {
            if let Some(remaining) = self.player.remaining() {
                if remaining > Duration::ZERO && remaining <= Duration::from_secs(2) {
                    let cur = self.queue_index.unwrap_or(0);
                    if let Some(next_idx) = self.pick_next_index(cur) {
                        if let Some(track) = self.queue.get(next_idx).cloned() {
                            if self.player.enqueue_next(&track).is_ok() {
                                self.pending_gapless_idx = Some(next_idx);
                                self.player.rg_scale = rg_scale(&track, self.replaygain_mode);
                            }
                        }
                    }
                }
            }
        }

        // Radio Mode (#56) — kick off a refill when the upcoming queue runs low and
        // collect the result. The fetch reuses ytdlp::fetch_tracks (which now has
        // retry from #69), so transient YouTube failures don't kill the stream.
        if self.radio_mode.active && self.radio_fetch_rx.is_none() {
            let upcoming = self
                .queue
                .len()
                .saturating_sub(self.queue_index.map(|i| i + 1).unwrap_or(0));
            if self.radio_mode.should_fetch(upcoming) {
                let current_title = self.player.current().map(|t| t.title.clone());
                let query = self.radio_mode.query(current_title.as_deref());
                let (tx, rx) = std::sync::mpsc::channel();
                self.radio_fetch_rx = Some(rx);
                self.radio_mode.is_fetching = true;
                std::thread::spawn(move || {
                    let res = crate::ytdlp::fetch_tracks(&query).map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
            }
        }
        if let Some(rx) = &self.radio_fetch_rx {
            match rx.try_recv() {
                Ok(Ok(mut tracks)) => {
                    let n = tracks.len();
                    self.queue.append(&mut tracks);
                    self.set_info(format!("Radio: +{n} tracks."));
                    self.radio_mode.is_fetching = false;
                    self.radio_fetch_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Radio: fetch failed — {e}"));
                    self.radio_mode.is_fetching = false;
                    self.radio_fetch_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.radio_mode.is_fetching = false;
                    self.radio_fetch_rx = None;
                }
            }
        }

        // Record play after 30s threshold (once per track start)
        if !self.current_play_recorded {
            if let Some(track) = self.player.current().cloned() {
                if self.player.elapsed().as_secs_f64() >= self.play_threshold_secs {
                    self.play_history.record_play(&track.path);
                    self.play_history_revision = self.play_history_revision.wrapping_add(1);
                    self.current_play_recorded = true;

                    // Scrobble to Last.fm (once per track)
                    if !self.lastfm_scrobbled {
                        if let (Some(lfm), Some((artist, title, ts))) =
                            (self.lastfm.clone(), self.lastfm_scrobble_info.clone())
                        {
                            self.lastfm_scrobbled = true;
                            std::thread::spawn(move || {
                                let _ = lfm.scrobble(&artist, &title, ts);
                            });
                        }
                    }
                }
            }
        }

        // Regular end detection (covers repeat:one and tracks with no duration)
        if self.player.is_empty() && self.player.current().is_some() {
            self.advance();
        }
        Ok(())
    }

    pub fn sleep_remaining(&self) -> Option<Duration> {
        self.sleep_until
            .map(|t| t.saturating_duration_since(std::time::Instant::now()))
    }

    fn toggle_sleep_timer(&mut self) {
        if self.sleep_until.is_some() {
            self.sleep_until = None;
            self.set_info("Sleep timer cancelled.");
        } else {
            let when = std::time::Instant::now() + Duration::from_secs(30 * 60);
            self.sleep_until = Some(when);
            self.set_info("Sleep timer: 30 min.");
        }
    }

    fn push_history(&mut self, t: Track) {
        if self
            .history
            .front()
            .map(|h| h.path == t.path)
            .unwrap_or(false)
        {
            return;
        }
        self.history.push_front(t);
        while self.history.len() > 64 {
            self.history.pop_back();
        }
        self.history_revision = self.history_revision.wrapping_add(1);
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return;
        }

        if self.show_help {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                _ => {
                    self.show_help = false;
                    self.help_scroll = 0;
                }
            }
            return;
        }
        if self.show_info {
            self.show_info = false;
            return;
        }

        if self.show_audio_panel {
            self.handle_audio_panel_key(key);
            return;
        }

        if self.show_device_selector {
            self.handle_device_selector_key(key);
            return;
        }

        if self.show_eq_tuner {
            self.handle_eq_tuner_key(key);
            return;
        }

        if self.show_playlist_browser {
            self.handle_playlist_browser_key(key);
            return;
        }

        if self.show_profile_browser {
            self.handle_profile_browser_key(key);
            return;
        }

        if self.show_spotify_browser {
            self.handle_spotify_browser_key(key);
            return;
        }

        if self.show_tag_editor {
            self.handle_tag_editor_key(key);
            return;
        }

        if self.show_radio_browser {
            self.handle_radio_browser_key(key);
            return;
        }

        if self.eq_preset_name_editing {
            match key.code {
                KeyCode::Esc => {
                    self.eq_preset_name_input.clear();
                    self.eq_preset_name_editing = false;
                }
                KeyCode::Enter => {
                    let name = self.eq_preset_name_input.trim().to_string();
                    self.eq_preset_name_input.clear();
                    self.eq_preset_name_editing = false;
                    self.save_eq_preset(name);
                }
                KeyCode::Backspace => {
                    self.eq_preset_name_input.pop();
                }
                KeyCode::Char(c) => {
                    self.eq_preset_name_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.profile_name_editing {
            match key.code {
                KeyCode::Esc => {
                    self.profile_name_input.clear();
                    self.profile_name_editing = false;
                }
                KeyCode::Enter => {
                    let name = self.profile_name_input.trim().to_string();
                    self.profile_name_input.clear();
                    self.profile_name_editing = false;
                    self.save_profile(name);
                }
                KeyCode::Backspace => {
                    self.profile_name_input.pop();
                }
                KeyCode::Char(c) => {
                    self.profile_name_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.radio_seed_editing {
            match key.code {
                KeyCode::Esc => {
                    self.radio_seed_input.clear();
                    self.radio_seed_editing = false;
                    self.set_info("Radio cancelled.");
                }
                KeyCode::Enter => {
                    let seed = self.radio_seed_input.trim().to_string();
                    self.radio_seed_editing = false;
                    self.radio_seed_input.clear();
                    if seed.is_empty() {
                        self.set_info("Radio: empty seed.");
                    } else {
                        self.radio_mode.seed = seed.clone();
                        self.radio_mode.active = true;
                        self.set_info(format!("Radio: '{seed}' — fetching first batch…"));
                    }
                }
                KeyCode::Backspace => {
                    self.radio_seed_input.pop();
                }
                KeyCode::Char(c) => {
                    self.radio_seed_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.playlist_name_editing {
            match key.code {
                KeyCode::Esc => {
                    self.playlist_name_input.clear();
                    self.playlist_name_editing = false;
                }
                KeyCode::Enter => {
                    let name = self.playlist_name_input.trim().to_string();
                    self.playlist_name_input.clear();
                    self.playlist_name_editing = false;
                    self.save_playlist_named(name);
                }
                KeyCode::Backspace => {
                    self.playlist_name_input.pop();
                }
                KeyCode::Char(c) => {
                    self.playlist_name_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.url_editing {
            match key.code {
                KeyCode::Esc => {
                    self.url_input.clear();
                    self.url_editing = false;
                }
                KeyCode::Enter => {
                    let url = self.url_input.trim().to_string();
                    self.url_input.clear();
                    self.url_editing = false;
                    if !url.is_empty() {
                        self.start_url_load(url);
                    }
                }
                KeyCode::Backspace => {
                    self.url_input.pop();
                }
                KeyCode::Char(c) => {
                    self.url_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.search_editing {
            match key.code {
                KeyCode::Esc => {
                    self.search.clear();
                    self.search_editing = false;
                    self.library_state.select(if self.library.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                }
                KeyCode::Enter => {
                    self.search_editing = false;
                    self.library_state.select(Some(0));
                }
                KeyCode::Backspace => {
                    self.search.pop();
                    self.library_state.select(Some(0));
                }
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.library_state.select(Some(0));
                }
                _ => {}
            }
            return;
        }

        // Browser: Backspace goes up a directory
        if self.view_mode == ViewMode::Browser
            && self.focus == Pane::Library
            && key.code == KeyCode::Backspace
        {
            self.browser_up();
            return;
        }

        // Radio View Search input active
        if self.view_mode == ViewMode::Radio && self.radio_search_editing {
            match key.code {
                KeyCode::Esc => {
                    self.radio_search_editing = false;
                }
                KeyCode::Enter => {
                    self.radio_search_editing = false;
                    self.trigger_radio_search();
                }
                KeyCode::Backspace => {
                    self.radio_search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.radio_search_query.push(c);
                }
                _ => {}
            }
            return;
        }

        // Radio View keyboard navigation
        if self.view_mode == ViewMode::Radio && self.focus == Pane::Library {
            match key.code {
                KeyCode::Tab => {
                    self.radio_focus_pane = (self.radio_focus_pane + 1) % 2;
                    return;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.radio_focus_pane == 0 {
                        self.radio_category_idx = self.radio_category_idx.saturating_sub(1);
                        self.radio_row = 0;
                    } else {
                        self.radio_row = self.radio_row.saturating_sub(1);
                    }
                    return;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.radio_focus_pane == 0 {
                        let max_cat = crate::radio_browser::RadioCategory::ALL.len().saturating_sub(1);
                        if self.radio_category_idx < max_cat {
                            self.radio_category_idx += 1;
                            self.radio_row = 0;
                        }
                    } else {
                        let count = self.radio_filtered_stations().len();
                        if count > 0 && self.radio_row + 1 < count {
                            self.radio_row += 1;
                        }
                    }
                    return;
                }
                KeyCode::Enter => {
                    if self.radio_focus_pane == 0 {
                        if self.radio_category_idx == 8 {
                            // Search category
                            self.radio_search_editing = true;
                        } else {
                            self.radio_focus_pane = 1;
                        }
                    } else {
                        let stations = self.radio_filtered_stations();
                        if let Some(&st) = stations.get(self.radio_row) {
                            let st_clone = st.clone();
                            self.play_radio_station(&st_clone, false);
                        }
                    }
                    return;
                }
                KeyCode::Char('a') => {
                    let stations = self.radio_filtered_stations();
                    if let Some(&st) = stations.get(self.radio_row) {
                        let st_clone = st.clone();
                        self.play_radio_station(&st_clone, true);
                    }
                    return;
                }
                KeyCode::Char('f') => {
                    let stations = self.radio_filtered_stations();
                    if let Some(&st) = stations.get(self.radio_row) {
                        let p = std::path::PathBuf::from(&st.url);
                        let fav = self.ratings.toggle_favorite(&p);
                        self.set_info(if fav {
                            "Rádio adicionada aos favoritos ♥"
                        } else {
                            "Rádio removida dos favoritos"
                        });
                    }
                    return;
                }
                KeyCode::Char('/') => {
                    self.radio_category_idx = 8; // Search
                    self.radio_search_editing = true;
                    return;
                }
                _ => {}
            }
        }

        if let Some(action) = self.bindings.lookup(key.code, key.modifiers) {
            self.run_action(action);
        } else {
            match key.code {
                KeyCode::Char('=') => self.run_action(Action::VolumeUp),
                KeyCode::Char('_') => self.run_action(Action::VolumeDown),
                _ => {}
            }
        }
    }

    fn run_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Help => self.show_help = true,
            Action::Search => {
                self.search_editing = true;
                self.focus = Pane::Library;
            }
            Action::Tab => {
                self.focus = match self.focus {
                    Pane::Library => Pane::Queue,
                    Pane::Queue => Pane::Library,
                };
            }
            Action::PlayPause => self.player.toggle(),
            Action::Next => self.next(),
            Action::Prev => self.prev(),
            Action::Stop => {
                self.player.stop();
                if let Some(tx) = &self.discord_tx {
                    let _ = tx.send(crate::discord::Cmd::Clear);
                }
            }
            Action::Shuffle => {
                self.shuffle = !self.shuffle;
                self.update_prefetch_slots();
                self.set_info(format!(
                    "Shuffle: {}",
                    if self.shuffle { "on" } else { "off" }
                ));
            }
            Action::Repeat => {
                self.repeat = self.repeat.cycle();
                self.update_prefetch_slots();
                self.set_info(format!("Repeat: {}", self.repeat.label()));
            }
            Action::Sort => {
                self.sort = self.sort.cycle();
                sort_tracks_with_ratings(&mut self.library, self.sort, Some(&self.ratings));
                self.library_state.select(if self.library.is_empty() {
                    None
                } else {
                    Some(0)
                });
                self.set_info(format!("Sort: {}", self.sort.label()));
            }
            Action::SleepTimer => self.toggle_sleep_timer(),
            Action::SavePlaylist => {
                self.playlist_name_editing = true;
                self.playlist_name_input.clear();
                self.set_info("Playlist name (Enter to save, Esc to cancel):");
            }
            Action::LoadPlaylist => {
                self.open_playlist_browser();
            }
            Action::Profiles => {
                self.show_profile_browser = true;
                self.profile_browser_row = 0;
            }
            Action::ShowStats => {
                self.show_stats = !self.show_stats;
            }
            Action::LastfmPanel => {
                if !self.show_lastfm_panel {
                    if let Some(lfm) = self.lastfm.clone() {
                        let (tx, rx) = std::sync::mpsc::channel();
                        self.lastfm_panel_rx = Some(rx);
                        std::thread::spawn(move || {
                            let recent = lfm.recent_tracks(10).unwrap_or_default();
                            let top = lfm.top_artists("1month", 10).unwrap_or_default();
                            let _ = tx.send((recent, top));
                        });
                        self.show_lastfm_panel = true;
                        self.set_info("Last.fm: loading…");
                    } else {
                        self.set_error("Last.fm: not logged in — press Shift+F to authenticate");
                    }
                } else {
                    self.show_lastfm_panel = false;
                }
            }
            Action::RadioMode => {
                if self.radio_mode.active {
                    self.radio_mode.active = false;
                    self.set_info("Radio Mode off.");
                } else {
                    self.radio_seed_editing = true;
                    self.radio_seed_input = self.radio_mode.seed.clone();
                    self.set_info("Radio seed (Enter=confirm, Esc=cancel):");
                }
            }
            Action::SpotifyBrowser => {
                if self.spotify.is_none() {
                    self.set_error("Spotify: not authorized — press Shift+P to login");
                } else {
                    self.show_spotify_browser = true;
                    self.spotify_browser_tab = SpotifyTab::Search;
                    self.spotify_browser_query_editing = true;
                    if self.spotify_my_playlists.is_empty() {
                        self.spotify_load_my_playlists();
                    }
                }
            }
            Action::VolumeUp => {
                let v = (self.player.volume() + 0.05).min(1.5);
                self.player.set_volume(v);
                self.config.playback.default_volume = v;
            }
            Action::VolumeDown => {
                let v = (self.player.volume() - 0.05).max(0.0);
                self.player.set_volume(v);
                self.config.playback.default_volume = v;
            }
            Action::SeekBack => {
                self.seek_relative_async(-5);
            }
            Action::SeekForward => {
                self.seek_relative_async(5);
            }
            Action::SelectionUp => self.move_selection(-1),
            Action::SelectionDown => self.move_selection(1),
            Action::ActivateSelection => self.activate_selection(),
            Action::Enqueue => self.enqueue_selection(),
            Action::RemoveQueueItem => self.remove_from_queue(),
            Action::ClearQueue => self.clear_queue(),
            Action::SpotifyLogin => self.spotify_login(),
            Action::SpotifyToggle => self.spotify_toggle(),
            Action::ToggleView => {
                self.view_mode = self.view_mode.toggle();
                if self.view_mode == ViewMode::Browser {
                    self.browser_path = None;
                    self.browser_music_root_idx = 0;
                }
                self.library_state.select(Some(0));
                self.set_info(format!("View: {}", self.view_mode.label()));
            }
            Action::EqLowUp => {
                self.player.eq().adjust_low(1.0);
                self.set_info("EQ low +1 dB");
            }
            Action::EqLowDown => {
                self.player.eq().adjust_low(-1.0);
                self.set_info("EQ low -1 dB");
            }
            Action::EqMidUp => {
                self.player.eq().adjust_mid(1.0);
                self.set_info("EQ mid +1 dB");
            }
            Action::EqMidDown => {
                self.player.eq().adjust_mid(-1.0);
                self.set_info("EQ mid -1 dB");
            }
            Action::EqHighUp => {
                self.player.eq().adjust_high(1.0);
                self.set_info("EQ high +1 dB");
            }
            Action::EqHighDown => {
                self.player.eq().adjust_high(-1.0);
                self.set_info("EQ high -1 dB");
            }
            Action::OpenUrl => {
                self.url_editing = true;
                self.status =
                    "URL/search — YouTube, ytmsearch:..., Spotify, radio M3U/PLS — Enter/Esc"
                        .into();
            }
            Action::EqPreset => {
                let presets = crate::eq::PRESETS;
                self.eq_preset_idx = (self.eq_preset_idx + 1) % presets.len();
                let (name, state) = presets[self.eq_preset_idx];
                self.player.eq().set(state);
                self.set_info(format!("EQ preset: {name}"));
            }
            Action::Rescan => self.start_async_scan(),
            Action::TrackInfo => self.show_info = true,
            Action::CycleTheme => self.cycle_theme(),
            Action::VizSensUp => self.adjust_viz_sensitivity(crate::visualizer::SENS_STEP),
            Action::VizSensDown => self.adjust_viz_sensitivity(-crate::visualizer::SENS_STEP),
            Action::UndoQueue => self.undo_queue_action(),
            Action::RecentlyPlayed => {
                if self.view_mode == ViewMode::RecentlyPlayed {
                    self.view_mode = ViewMode::Flat;
                    self.set_info("View: library");
                } else {
                    self.view_mode = ViewMode::RecentlyPlayed;
                    self.set_info(format!("Recently played ({} tracks)", self.history.len()));
                }
                self.library_state
                    .select(if self.visible_library().is_empty() {
                        None
                    } else {
                        Some(0)
                    });
            }
            Action::ShowAudioPanel => {
                self.show_audio_panel = !self.show_audio_panel;
                self.audio_panel_row = 0;
            }
            Action::ReplayGain => {
                self.replaygain_mode = self.replaygain_mode.cycle();
                self.set_info(format!("ReplayGain: {}", self.replaygain_mode.label()));
            }
            Action::CycleVizMode => {
                self.viz_mode = self.viz_mode.cycle();
                self.set_info(format!("Visualizer: {}", self.viz_mode.label()));
            }
            Action::ToggleMini => {
                self.mini_mode = !self.mini_mode;
            }
            Action::LastfmLogin => self.lastfm_login(),
            Action::SelectDevice => self.open_device_selector(),
            Action::EqTuner => {
                self.show_eq_tuner = !self.show_eq_tuner;
                self.eq_tuner_band = 0;
            }
            Action::ToggleFavorite => {
                let path = match self.focus {
                    Pane::Library => self.selected_library_track().map(|t| t.path),
                    Pane::Queue => self
                        .queue_state
                        .selected()
                        .and_then(|i| self.queue.get(i))
                        .map(|t| t.path.clone()),
                };
                if let Some(p) = path {
                    let fav = self.ratings.toggle_favorite(&p);
                    self.set_info(if fav {
                        "Added to favorites ♥"
                    } else {
                        "Removed from favorites"
                    });
                }
            }
            Action::EditTags => self.open_tag_editor(),
            Action::RadioBrowser => {
                self.view_mode = ViewMode::Radio;
                self.focus = Pane::Library;
                self.set_info("📻 Radio Mode — Tab switch pane · Enter play · / search");
            }
            Action::SelfUpdate => self.handle_self_update(),
            Action::ViewLibrary => {
                self.view_mode = ViewMode::Flat;
                self.focus = Pane::Library;
                self.set_info("View: Library (1)");
            }
            Action::ViewQueue => {
                self.focus = Pane::Queue;
                self.set_info("Focus: Queue (2)");
            }
            Action::ViewRadio => {
                self.view_mode = ViewMode::Radio;
                self.focus = Pane::Library;
                self.set_info("📻 Radio Mode (3) — Tab switch pane · Enter play · / search");
            }
            Action::ViewBrowser => {
                self.view_mode = ViewMode::Browser;
                self.focus = Pane::Library;
                self.set_info("View: Folders Browser (4)");
            }
        }
    }

    fn adjust_viz_sensitivity(&mut self, delta: f32) {
        let new_val = self.tap.adjust_sensitivity(delta);
        self.config.visualizer.sensitivity = new_val;
        self.set_info(format!("Visualizer sensitivity: ×{:.1}", new_val));
        if let Err(e) = self.config.save() {
            self.set_info(format!("sensitivity saved in memory only ({e})"));
        }
    }

    pub const AUDIO_PANEL_ROWS: usize = 8;

    fn handle_audio_panel_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('e') => {
                self.show_audio_panel = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.audio_panel_row = self.audio_panel_row.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.audio_panel_row + 1 < Self::AUDIO_PANEL_ROWS =>
            {
                self.audio_panel_row += 1;
            }
            KeyCode::Left => self.audio_panel_adjust(-1),
            KeyCode::Right => self.audio_panel_adjust(1),
            _ => {}
        }
    }

    fn audio_panel_adjust(&mut self, dir: i32) {
        match self.audio_panel_row {
            0 => {
                self.player.eq().adjust_low(dir as f32);
                let db = self.player.eq().snapshot().low_db;
                self.set_info(format!("EQ Low: {:+.0} dB", db));
            }
            1 => {
                self.player.eq().adjust_mid(dir as f32);
                let db = self.player.eq().snapshot().mid_db;
                self.set_info(format!("EQ Mid: {:+.0} dB", db));
            }
            2 => {
                self.player.eq().adjust_high(dir as f32);
                let db = self.player.eq().snapshot().high_db;
                self.set_info(format!("EQ High: {:+.0} dB", db));
            }
            3 => {
                let presets = crate::eq::PRESETS;
                self.eq_preset_idx = if dir > 0 {
                    (self.eq_preset_idx + 1) % presets.len()
                } else {
                    self.eq_preset_idx.wrapping_sub(1).min(presets.len() - 1)
                };
                let (name, state) = presets[self.eq_preset_idx];
                self.player.eq().set(state);
                self.set_info(format!("EQ preset: {name}"));
            }
            4 => {
                let v = (self.player.volume() + dir as f32 * 0.05).clamp(0.0, 1.5);
                self.player.set_volume(v);
                self.config.playback.default_volume = v;
                self.set_info(format!("Volume: {}%", (v * 100.0) as u32));
            }
            5 => {
                let xf = (self.player.crossfade_secs + dir as f32 * 0.5).clamp(0.0, 10.0);
                self.player.crossfade_secs = xf;
                self.set_info(format!("Crossfade: {:.1}s", xf));
            }
            6 => {
                self.adjust_viz_sensitivity(dir as f32 * crate::visualizer::SENS_STEP);
            }
            7 => {
                let s = (self.player.speed() + dir as f32 * 0.05).clamp(0.5, 2.5);
                self.player.set_speed(s);
                self.set_info(format!("Playback speed: {:.2}×", s));
            }
            _ => {}
        }
    }

    fn cycle_theme(&mut self) {
        if self.theme_names.is_empty() {
            let dir = match crate::config::themes_dir() {
                Ok(d) => d,
                Err(e) => {
                    self.set_info(format!("themes dir: {e}"));
                    return;
                }
            };
            let mut names: Vec<String> = std::fs::read_dir(&dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("toml") {
                        p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            if names.is_empty() {
                names.push("default".to_string());
            }
            self.theme_idx = names
                .iter()
                .position(|n| n == &self.theme.name)
                .unwrap_or(0);
            self.theme_names = names;
        }
        if self.theme_names.is_empty() {
            return;
        }
        self.theme_idx = (self.theme_idx + 1) % self.theme_names.len();
        let name = &self.theme_names[self.theme_idx];
        match crate::theme::Theme::load(name) {
            Ok(t) => {
                self.theme = t.clone();
                self.config.theme = name.clone();
                // #68: persist so the next launch picks up the user's selection. Saving
                // is fire-and-forget; errors land in the status bar but do not interrupt
                // the cycle.
                if let Err(e) = self.config.save() {
                    self.set_info(format!("Theme: {name} (config save failed: {e})"));
                } else {
                    self.set_info(format!("Theme: {name} (saved)"));
                }
                self.rearm_theme_watcher();
            }
            Err(e) => self.set_info(format!("Theme load error: {e}")),
        }
    }

    /// (Re-)arm the filesystem watcher pointed at the currently active theme so edits
    /// from an external editor are picked up live (#68). Called on startup and whenever
    /// the theme switches.
    fn rearm_theme_watcher(&mut self) {
        let Ok(dir) = crate::config::themes_dir() else {
            return;
        };
        let path = dir.join(format!("{}.toml", self.theme.name));
        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut w = match notify::RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(w) => w,
            Err(_) => return,
        };
        if w.watch(&path, notify::RecursiveMode::NonRecursive).is_ok() {
            self.theme_watcher_rx = Some(rx);
            self._theme_watcher = Some(w);
        }
    }

    fn start_async_scan(&mut self) {
        if self.scan_rx.is_some() {
            self.set_info("Scan already in progress…");
            return;
        }
        let dirs = self.config.music_dirs.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<Track>>();
        let (ptx, prx) = std::sync::mpsc::channel::<(usize, usize)>();
        self.scan_rx = Some(rx);
        self.scan_progress_rx = Some(prx);
        self.scan_progress = None;
        self.set_info("Scanning library…");
        std::thread::spawn(move || {
            let cache_file = cache_path();
            let mut cache = cache_file
                .as_ref()
                .map(|p| MetadataCache::load(p))
                .unwrap_or_default();
            let tracks = scan_library_with_progress(&dirs, &mut cache, Some(ptx));
            if let Some(p) = &cache_file {
                cache.save(p);
            }
            let _ = tx.send(tracks);
        });
    }

    fn start_url_load(&mut self, url: String) {
        if self.url_rx.is_some() {
            self.set_info("Already loading, please wait…");
            return;
        }

        // Radio playlist (M3U / PLS) — fetch and parse synchronously, then enqueue streams
        let lower = url.to_lowercase();
        let is_playlist_file =
            (lower.ends_with(".m3u") || lower.ends_with(".m3u8") || lower.ends_with(".pls"))
                && !url.contains("spotify.com")
                && !url.starts_with("spotify:")
                && !crate::ytdlp::is_youtube_url(&url);

        if is_playlist_file {
            self.set_info("Loading radio playlist…");
            match crate::radio::fetch_playlist(&url) {
                Ok(tracks) if !tracks.is_empty() => {
                    let n = tracks.len();
                    let was_empty = self.queue.is_empty();
                    self.queue.extend(tracks);
                    if was_empty {
                        self.queue_state.select(Some(0));
                    }
                    self.set_info(format!("Added {n} stream(s) from playlist."));
                }
                Ok(_) => self.set_info("Playlist contained no playable streams."),
                Err(e) => self.set_info(format!("Playlist error: {e}")),
            }
            return;
        }

        // Plain HTTP stream (instant — no thread needed)
        if !url.contains("spotify.com")
            && !url.starts_with("spotify:")
            && !crate::ytdlp::is_youtube_url(&url)
            && (url.starts_with("http://") || url.starts_with("https://"))
        {
            self.queue.push(crate::audio::Track::from_url(url));
            if self.queue_state.selected().is_none() {
                self.queue_state
                    .select(Some(self.queue.len().saturating_sub(1)));
            }
            self.set_info("Added stream URL to queue.");
            return;
        }

        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && !url.starts_with("spotify:")
            && !crate::ytdlp::is_youtube_url(&url)
        {
            self.set_info(format!("Unrecognised URL scheme: {url}"));
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);

        // Spotify
        if url.contains("spotify.com") || url.starts_with("spotify:") {
            let Some(api) = self.spotify.clone() else {
                self.set_error("Spotify: not authorized — press Shift+P to login");
                self.url_rx = None;
                return;
            };
            let (kind, id) = parse_spotify_url(&url);
            self.set_info(format!("Loading Spotify {kind}…"));
            std::thread::spawn(move || {
                let mut api = api;
                let result = match kind.as_str() {
                    "track" => api.track_by_id(&id).map(|t| vec![t]),
                    "playlist" => api.playlist_tracks(&id),
                    "album" => api.album_tracks(&id),
                    other => Err(anyhow::anyhow!("Unsupported Spotify type: {other}")),
                };
                let _ = tx.send(result.map_err(|e| e.to_string()));
            });
            return;
        }

        // YouTube / YT Music
        self.set_info(format!("Loading {url}…"));
        std::thread::spawn(move || {
            let result = crate::ytdlp::fetch_tracks(&url)
                .map_err(|e| e.to_string())
                .and_then(|tracks| {
                    if tracks.is_empty() {
                        Err("yt-dlp returned no tracks for that URL.".into())
                    } else {
                        Ok(tracks)
                    }
                });
            let _ = tx.send(result);
        });
    }

    fn spotify_login(&mut self) {
        if self.spotify_client_id.is_empty() {
            self.set_info("Set [spotify].client_id in config.toml first.");
            return;
        }
        match crate::spotify::authorize(&self.spotify_client_id, &self.spotify_redirect_uri) {
            Ok((url, session)) => {
                self.set_info("Opening browser for Spotify login...");
                let _ = webbrowser::open(&url);
                let port = self
                    .spotify_redirect_uri
                    .rsplit(':')
                    .next()
                    .and_then(|s| s.split('/').next())
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(8888);
                match crate::spotify::oauth::wait_for_redirect(port, &session.state) {
                    Ok(code) => match crate::spotify::exchange_code(&session, &code) {
                        Ok(tokens) => {
                            let _ = crate::spotify::save_tokens(&tokens);
                            match crate::spotify::SpotifyApi::new(
                                self.spotify_client_id.clone(),
                                tokens,
                            ) {
                                Ok(api) => {
                                    self.spotify = Some(api);
                                    self.set_info("Spotify login complete.");
                                }
                                Err(e) => self.set_info(format!("Spotify init error: {e}")),
                            }
                        }
                        Err(e) => self.set_info(format!("Token exchange failed: {e}")),
                    },
                    Err(e) => self.set_info(format!("Redirect listener error: {e}")),
                }
            }
            Err(e) => self.set_info(format!("Spotify authorize error: {e}")),
        }
    }

    fn open_tag_editor(&mut self) {
        let track = match self.focus {
            Pane::Library => self.selected_library_track(),
            Pane::Queue => self
                .queue_state
                .selected()
                .and_then(|i| self.queue.get(i))
                .cloned(),
        };
        let Some(t) = track else {
            self.set_info("No track selected to edit tags.");
            return;
        };
        if Self::track_is_stream(&t) {
            self.set_info("Cannot edit tags on stream URLs.");
            return;
        }

        self.tag_editor_path = Some(t.path.clone());
        self.tag_editor_fields = [
            t.title.clone(),
            t.artist.unwrap_or_default(),
            t.album.unwrap_or_default(),
            t.genre.unwrap_or_default(),
            t.year.unwrap_or_default(),
        ];
        self.tag_editor_row = 0;
        self.show_tag_editor = true;
    }

    fn handle_tag_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_tag_editor = false;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.tag_editor_row = (self.tag_editor_row + 1) % 5;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.tag_editor_row = if self.tag_editor_row == 0 { 4 } else { self.tag_editor_row - 1 };
            }
            KeyCode::Enter => {
                self.save_tag_editor();
                self.show_tag_editor = false;
            }
            KeyCode::Backspace => {
                self.tag_editor_fields[self.tag_editor_row].pop();
            }
            KeyCode::Char(c) => {
                self.tag_editor_fields[self.tag_editor_row].push(c);
            }
            _ => {}
        }
    }

    fn save_tag_editor(&mut self) {
        let Some(path) = self.tag_editor_path.take() else { return };
        let title = self.tag_editor_fields[0].trim().to_string();
        let artist = if self.tag_editor_fields[1].trim().is_empty() { None } else { Some(self.tag_editor_fields[1].trim().to_string()) };
        let album = if self.tag_editor_fields[2].trim().is_empty() { None } else { Some(self.tag_editor_fields[2].trim().to_string()) };
        let genre = if self.tag_editor_fields[3].trim().is_empty() { None } else { Some(self.tag_editor_fields[3].trim().to_string()) };
        let year = if self.tag_editor_fields[4].trim().is_empty() { None } else { Some(self.tag_editor_fields[4].trim().to_string()) };

        // Update in Library
        for t in &mut self.library {
            if t.path == path {
                t.title = if title.is_empty() { t.title.clone() } else { title.clone() };
                t.artist = artist.clone();
                t.album = album.clone();
                t.genre = genre.clone();
                t.year = year.clone();
            }
        }
        // Update in Queue
        for t in &mut self.queue {
            if t.path == path {
                t.title = if title.is_empty() { t.title.clone() } else { title.clone() };
                t.artist = artist.clone();
                t.album = album.clone();
                t.genre = genre.clone();
                t.year = year.clone();
            }
        }
        self.library_revision = self.library_revision.wrapping_add(1);
        self.set_info(format!("Tags updated: {}", title));
    }

    fn open_radio_browser(&mut self) {
        self.show_radio_browser = true;
        self.radio_row = 0;
        self.radio_search_editing = false;
        self.set_info("Radio Hub — Tab switch mode · Enter play · a enqueue · / search");
    }

    fn trigger_radio_search(&mut self) {
        let q = self.radio_search_query.trim().to_string();
        if q.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.radio_search_rx = Some(rx);
        self.set_info(format!("Searching Radio-Browser for '{q}'…"));
        std::thread::spawn(move || {
            let res = crate::radio_browser::search_radio_browser(&q, 50).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    fn handle_radio_browser_key(&mut self, key: KeyEvent) {
        if self.radio_search_editing {
            match key.code {
                KeyCode::Esc => {
                    self.radio_search_editing = false;
                }
                KeyCode::Enter => {
                    self.radio_search_editing = false;
                    self.trigger_radio_search();
                }
                KeyCode::Backspace => {
                    self.radio_search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.radio_search_query.push(c);
                }
                _ => {}
            }
            return;
        }

        let list_len = match self.radio_tab {
            crate::radio_browser::RadioTab::Curated => self.radio_curated_list.len(),
            crate::radio_browser::RadioTab::Search => self.radio_search_results.len(),
        };

        match key.code {
            KeyCode::Esc | KeyCode::Char('K') => {
                self.show_radio_browser = false;
                self.radio_search_editing = false;
            }
            KeyCode::Tab => {
                self.radio_tab = self.radio_tab.cycle();
                self.radio_row = 0;
            }
            KeyCode::Char('/') => {
                self.radio_tab = crate::radio_browser::RadioTab::Search;
                self.radio_search_editing = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.radio_row = self.radio_row.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if list_len > 0 && self.radio_row + 1 < list_len {
                    self.radio_row += 1;
                }
            }
            KeyCode::Enter => {
                let station = match self.radio_tab {
                    crate::radio_browser::RadioTab::Curated => self.radio_curated_list.get(self.radio_row).cloned(),
                    crate::radio_browser::RadioTab::Search => self.radio_search_results.get(self.radio_row).cloned(),
                };
                if let Some(st) = station {
                    self.play_radio_station(&st, false);
                }
            }
            KeyCode::Char('a') => {
                let station = match self.radio_tab {
                    crate::radio_browser::RadioTab::Curated => self.radio_curated_list.get(self.radio_row).cloned(),
                    crate::radio_browser::RadioTab::Search => self.radio_search_results.get(self.radio_row).cloned(),
                };
                if let Some(st) = station {
                    self.play_radio_station(&st, true);
                }
            }
            _ => {}
        }
    }

    fn play_radio_station(&mut self, station: &crate::radio_browser::RadioStation, enqueue: bool) {
        let mut track = Track::from_url(station.url.clone());
        track.title = station.name.clone();
        track.artist = Some("Radio Stream".into());
        track.genre = Some(station.tags.clone());

        if enqueue {
            let name = station.name.clone();
            self.queue.push(track);
            self.set_info(format!("Enqueued radio: {name}"));
        } else {
            let name = station.name.clone();
            self.queue.push(track);
            let idx = self.queue.len() - 1;
            self.queue_index = Some(idx);
            self.queue_state.select(Some(idx));
            self.play_current();
            self.set_info(format!("Playing radio: {name}"));
        }
    }

    fn handle_self_update(&mut self) {
        if self.is_updating {
            self.set_info("Update already in progress, please wait…");
            return;
        }

        if let Some(info) = &self.update_info {
            if let Some(url) = info.download_url.clone() {
                self.is_updating = true;
                self.set_info(format!("Downloading Noctune v{}…", info.latest_version));
                let (tx, rx) = std::sync::mpsc::channel();
                self.update_apply_rx = Some(rx);
                std::thread::spawn(move || {
                    let res = crate::updater::apply_update(&url).map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
            } else {
                self.set_info(format!(
                    "No automatic binary available for this platform. Please check GitHub release v{}.",
                    info.latest_version
                ));
            }
        } else {
            self.set_info("Checking for new Noctune updates…");
            let (tx, rx) = std::sync::mpsc::channel();
            self.update_check_rx = Some(rx);
            std::thread::spawn(move || {
                let res = crate::updater::check_for_updates().map_err(|e| e.to_string());
                let _ = tx.send(res);
            });
        }
    }

    pub fn radio_filtered_stations(&self) -> Vec<&crate::radio_browser::RadioStation> {
        let cat = crate::radio_browser::RadioCategory::ALL
            .get(self.radio_category_idx)
            .copied()
            .unwrap_or(crate::radio_browser::RadioCategory::All);

        match cat {
            crate::radio_browser::RadioCategory::All => self.radio_curated_list.iter().collect(),
            crate::radio_browser::RadioCategory::Favorites => self
                .radio_curated_list
                .iter()
                .chain(self.radio_search_results.iter())
                .filter(|st| self.ratings.is_favorite(&PathBuf::from(&st.url)))
                .collect(),
            crate::radio_browser::RadioCategory::Lofi => self
                .radio_curated_list
                .iter()
                .filter(|st| {
                    let t = st.tags.to_lowercase();
                    t.contains("lofi") || t.contains("chill") || t.contains("study") || t.contains("beats")
                })
                .collect(),
            crate::radio_browser::RadioCategory::Jazz => self
                .radio_curated_list
                .iter()
                .filter(|st| {
                    let t = st.tags.to_lowercase();
                    t.contains("jazz") || t.contains("blues") || t.contains("swing") || t.contains("lounge")
                })
                .collect(),
            crate::radio_browser::RadioCategory::Synthwave => self
                .radio_curated_list
                .iter()
                .filter(|st| {
                    let t = st.tags.to_lowercase();
                    t.contains("synthwave") || t.contains("retrowave") || t.contains("cyber") || t.contains("hacker") || t.contains("darkwave")
                })
                .collect(),
            crate::radio_browser::RadioCategory::Rock => self
                .radio_curated_list
                .iter()
                .filter(|st| {
                    let t = st.tags.to_lowercase();
                    t.contains("rock") || t.contains("metal") || t.contains("indie") || t.contains("alternative")
                })
                .collect(),
            crate::radio_browser::RadioCategory::Brazil => self
                .radio_curated_list
                .iter()
                .filter(|st| {
                    let t = st.tags.to_lowercase();
                    let c = st.country.as_deref().unwrap_or("").to_lowercase();
                    c.contains("brazil") || c.contains("brasil") || t.contains("mpb") || t.contains("bossa")
                })
                .collect(),
            crate::radio_browser::RadioCategory::Classical => self
                .radio_curated_list
                .iter()
                .filter(|st| {
                    let t = st.tags.to_lowercase();
                    t.contains("classical") || t.contains("piano") || t.contains("baroque") || t.contains("orchestral") || t.contains("opera")
                })
                .collect(),
            crate::radio_browser::RadioCategory::Search => self.radio_search_results.iter().collect(),
        }
    }

    fn open_device_selector(&mut self) {
        self.device_list = crate::audio::enumerate_output_devices();
        let default = crate::audio::default_device_name().unwrap_or_default();
        self.device_selector_row = self
            .device_list
            .iter()
            .position(|n| *n == default)
            .unwrap_or(0);
        self.show_device_selector = true;
    }

    fn handle_device_selector_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('D') => {
                self.show_device_selector = false;
            }
            KeyCode::Up | KeyCode::Char('k') if self.device_selector_row > 0 => {
                self.device_selector_row -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.device_selector_row + 1 < self.device_list.len() =>
            {
                self.device_selector_row += 1;
            }
            KeyCode::Enter => {
                self.show_device_selector = false;
                if let Some(name) = self.device_list.get(self.device_selector_row).cloned() {
                    match self.player.switch_device(&name) {
                        Ok(_) => self.set_info(format!("Output device: {name}")),
                        Err(e) => self.set_info(format!("Device error: {e}")),
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_eq_tuner_key(&mut self, key: KeyEvent) {
        // Ctrl+S: save current EQ as named preset
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.eq_preset_name_editing = true;
            self.eq_preset_name_input.clear();
            return;
        }

        let eq = self.player.eq();
        match key.code {
            KeyCode::Esc | KeyCode::Char('E') => {
                self.show_eq_tuner = false;
            }
            KeyCode::Left | KeyCode::Char('h') => match self.eq_tuner_band {
                0 => eq.adjust_low(-1.0),
                1 => eq.adjust_mid(-1.0),
                _ => eq.adjust_high(-1.0),
            },
            KeyCode::Right | KeyCode::Char('l') => match self.eq_tuner_band {
                0 => eq.adjust_low(1.0),
                1 => eq.adjust_mid(1.0),
                _ => eq.adjust_high(1.0),
            },
            KeyCode::Up | KeyCode::Char('k') if self.eq_tuner_band > 0 => {
                self.eq_tuner_band -= 1;
            }
            KeyCode::Down | KeyCode::Char('j') if self.eq_tuner_band < 2 => {
                self.eq_tuner_band += 1;
            }
            KeyCode::Char('0') => {
                // Cycle through built-in presets then custom presets
                let snap = eq.snapshot();
                let builtins = crate::eq::PRESETS;
                let all: Vec<(&str, crate::eq::EqState)> = builtins
                    .iter()
                    .map(|(n, s)| (*n, *s))
                    .chain(self.custom_eq_presets.iter().map(|p| {
                        (
                            p.name.as_str(),
                            crate::eq::EqState {
                                low_db: p.low_db,
                                mid_db: p.mid_db,
                                high_db: p.high_db,
                            },
                        )
                    }))
                    .collect();
                let next = all
                    .iter()
                    .position(|(_, s)| {
                        (s.low_db - snap.low_db).abs() < 0.1
                            && (s.mid_db - snap.mid_db).abs() < 0.1
                            && (s.high_db - snap.high_db).abs() < 0.1
                    })
                    .map(|i| (i + 1) % all.len())
                    .unwrap_or(0);
                eq.set(all[next].1);
                self.set_info(format!("EQ Preset: {}", all[next].0));
            }
            _ => {}
        }
    }

    fn lastfm_login(&mut self) {
        let cfg = &self.config.lastfm;
        if !cfg.is_configured() {
            self.set_info("Set [lastfm] api_key and api_secret in config.toml first.");
            return;
        }

        // If a pending token exists, complete the auth
        if let Some(token) = self.lastfm_pending_token.take() {
            let api_key = cfg.api_key.clone();
            let api_secret = cfg.api_secret.clone();
            match crate::lastfm::get_session(&api_key, &api_secret, &token) {
                Ok(session) => {
                    let username = session.username.clone();
                    crate::lastfm::save_session(&session);
                    match crate::lastfm::LastfmClient::new(api_key, api_secret, session) {
                        Ok(client) => {
                            self.lastfm = Some(client);
                            self.set_info(format!("Last.fm connected as {username}."));
                        }
                        Err(e) => self.set_info(format!("Last.fm client error: {e}")),
                    }
                }
                Err(e) => {
                    self.set_info(format!("Last.fm auth error: {e}"));
                }
            }
            return;
        }

        // Start new auth: get token and open browser
        let api_key = cfg.api_key.clone();
        let api_secret = cfg.api_secret.clone();
        match crate::lastfm::get_token(&api_key, &api_secret) {
            Ok(token) => {
                let url = format!(
                    "http://www.last.fm/api/auth/?api_key={}&token={}",
                    api_key, token
                );
                let _ = webbrowser::open(&url);
                self.lastfm_pending_token = Some(token);
                self.set_info("Last.fm: authorize in browser, then press F again.");
            }
            Err(e) => self.set_info(format!("Last.fm token error: {e}")),
        }
    }

    fn spotify_toggle(&mut self) {
        let Some(api) = self.spotify.as_mut() else {
            self.set_error("Spotify: not authorized — press Shift+P to login");
            return;
        };
        match api.currently_playing() {
            Ok(Some(cp)) if cp.is_playing => match api.pause() {
                Ok(_) => self.set_info("Spotify paused."),
                Err(e) => self.set_info(format!("Spotify pause error: {e}")),
            },
            Ok(_) => match api.play() {
                Ok(_) => self.set_info("Spotify resumed."),
                Err(e) => self.set_info(format!("Spotify play error: {e}")),
            },
            Err(e) => self.set_info(format!("Spotify error: {e}")),
        }
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.move_selection(-1),
            MouseEventKind::ScrollDown => self.move_selection(1),
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                self.handle_click(m.column, m.row);
            }
            MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                self.handle_drag(m.column, m.row);
            }
            MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                self.last_drag_seek = None;
            }
            MouseEventKind::Moved => {
                let prog = self.layout.progress;
                if m.row >= prog.y && m.row < prog.y + prog.height && prog.width > 0 {
                    self.hover_x = Some(m.column);
                } else {
                    self.hover_x = None;
                }
            }
            _ => {}
        }
    }

    fn handle_drag(&mut self, x: u16, y: u16) {
        let prog = self.layout.progress;
        if !rect_contains(prog, x, y) || prog.width == 0 {
            return;
        }
        // Debounce: max one seek every 120ms during a drag
        let now = std::time::Instant::now();
        if let Some(last) = self.last_drag_seek {
            if now.duration_since(last) < Duration::from_millis(50) {
                return;
            }
        }
        let frac = (x.saturating_sub(prog.x)) as f32 / prog.width as f32;
        if let Err(e) = self.seek_fraction_async(frac) {
            self.set_info(format!("Seek error: {e}"));
        }
        self.last_drag_seek = Some(now);
    }

    fn handle_click(&mut self, x: u16, y: u16) {
        let lib = self.layout.library;
        let q = self.layout.queue;
        let prog = self.layout.progress;

        if rect_contains(lib, x, y) {
            self.focus = Pane::Library;
            let row = (y - lib.y) as usize;
            let rows = self.library_rows();
            if let Some(item) = rows.get(row) {
                if matches!(item, LibraryRow::Track(_)) {
                    self.library_state.select(Some(row));
                    self.activate_selection();
                }
            }
            return;
        }
        if rect_contains(q, x, y) {
            self.focus = Pane::Queue;
            let row = (y - q.y) as usize;
            if row < self.queue.len() {
                self.queue_state.select(Some(row));
                self.queue_index = Some(row);
                self.play_current();
            }
            return;
        }
        if rect_contains(prog, x, y) && prog.width > 0 {
            let frac = (x.saturating_sub(prog.x)) as f32 / prog.width as f32;
            if let Err(e) = self.seek_fraction_async(frac) {
                self.set_info(format!("Seek error: {e}"));
            }
        }
    }

    fn move_selection(&mut self, delta: i32) {
        match self.focus {
            Pane::Library => {
                let rows = self.library_rows();
                if rows.is_empty() {
                    return;
                }
                let mut cur = self.library_state.selected().unwrap_or(0) as i32;
                let len = rows.len() as i32;
                for _ in 0..len {
                    cur = (cur + delta).rem_euclid(len);
                    match &rows[cur as usize] {
                        LibraryRow::Track(_)
                        | LibraryRow::SmartHeader { .. }
                        | LibraryRow::Dir(_) => {
                            self.library_state.select(Some(cur as usize));
                            return;
                        }
                        LibraryRow::Header(_) => {}
                    }
                }
            }
            Pane::Queue => {
                let len = self.queue.len();
                if len == 0 {
                    return;
                }
                let cur = self.queue_state.selected().unwrap_or(0) as i32;
                let new = (cur + delta).rem_euclid(len as i32) as usize;
                self.queue_state.select(Some(new));
            }
        }
    }

    fn activate_selection(&mut self) {
        match self.focus {
            Pane::Library => {
                if self.view_mode == ViewMode::Browser {
                    self.browser_enter();
                    return;
                }
                let sel = self.library_state.selected();
                let row = sel.and_then(|i| self.library_rows().into_iter().nth(i));
                if let Some(LibraryRow::SmartHeader { .. }) = row {
                    if let Some(i) = sel {
                        self.toggle_smart_category(i);
                    }
                    return;
                }
                if let Some(t) = self.selected_library_track() {
                    self.queue.push(t);
                    let idx = self.queue.len() - 1;
                    self.queue_index = Some(idx);
                    self.queue_state.select(Some(idx));
                    self.play_current();
                }
            }
            Pane::Queue => {
                if let Some(i) = self.queue_state.selected() {
                    self.queue_index = Some(i);
                    self.play_current();
                }
            }
        }
    }

    fn enqueue_selection(&mut self) {
        if self.focus == Pane::Library {
            if let Some(t) = self.selected_library_track() {
                self.set_info(format!("Queued: {}", t.display()));
                self.queue.push(t);
                if self.queue_state.selected().is_none() {
                    self.queue_state.select(Some(0));
                }
            }
        }
    }

    fn remove_from_queue(&mut self) {
        if self.focus == Pane::Queue {
            if let Some(i) = self.queue_state.selected() {
                if i < self.queue.len() {
                    let label = format!("removed '{}'", self.queue[i].display());
                    self.push_undo_snapshot(label);
                    self.queue.remove(i);
                    if self.queue_index == Some(i) {
                        self.queue_index = None;
                    } else if self.queue_index.is_some_and(|idx| idx > i) {
                        self.queue_index = self.queue_index.map(|idx| idx - 1);
                    }
                    if self.queue.is_empty() {
                        self.queue_state.select(None);
                    } else {
                        self.queue_state.select(Some(i.min(self.queue.len() - 1)));
                    }
                    self.update_prefetch_slots();
                }
            }
        }
    }

    fn clear_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        let confirmed = self
            .clear_confirm_until
            .map(|until| now < until)
            .unwrap_or(false);
        if !confirmed {
            self.clear_confirm_until = Some(now + Duration::from_secs(3));
            self.set_info(format!(
                "Press c again within 3s to clear {} tracks",
                self.queue.len()
            ));
            return;
        }
        self.clear_confirm_until = None;
        let n = self.queue.len();
        self.push_undo_snapshot(format!("cleared queue ({n} tracks)"));
        self.queue.clear();
        self.queue_state.select(None);
        self.queue_index = None;
        self.prefetch.invalidate();
        self.player.stop();
        self.album_art = None;
        self.art_generation = self.art_generation.wrapping_add(1);
        self.art_picker.invalidate();
        self.set_info(format!("Queue cleared ({n} tracks). Press u to undo."));
    }

    fn push_undo_snapshot(&mut self, label: String) {
        if self.undo_stack.len() >= MAX_UNDO_SNAPSHOTS {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(UndoSnapshot {
            queue: self.queue.clone(),
            queue_index: self.queue_index,
            label,
        });
    }

    fn undo_queue_action(&mut self) {
        let Some(snapshot) = self.undo_stack.pop_back() else {
            self.set_info("Nothing to undo.");
            return;
        };
        self.queue = snapshot.queue;
        self.queue_index = snapshot.queue_index.filter(|&i| i < self.queue.len());
        self.queue_state.select(if self.queue.is_empty() {
            None
        } else {
            Some(self.queue_index.unwrap_or(0).min(self.queue.len() - 1))
        });
        self.update_prefetch_slots();
        self.set_info(format!("Undo: {}", snapshot.label));
    }

    fn play_instant(&mut self, source: crate::audio::SymphoniaSource, track: Track) {
        self.current_play_recorded = false;
        self.lastfm_scrobbled = false;
        self.lastfm_scrobble_info = None;
        self.undo_stack.clear();
        self.load_rx = None;
        self.loading_track = None;
        self.pending_seek_offset = None;

        // Apply ReplayGain scaling
        self.player.rg_scale = rg_scale(&track, self.replaygain_mode);

        match self.player.play_prepared(source, &track, Duration::ZERO) {
            Ok(_) => {
                self.on_track_started(track);
            }
            Err(e) => {
                self.set_error(format!("Playback: {e}"));
                self.play_current();
            }
        }
    }

    fn play_current(&mut self) {
        self.current_play_recorded = false;
        self.lastfm_scrobbled = false;
        self.lastfm_scrobble_info = None;
        self.undo_stack.clear();
        self.prefetch.invalidate();
        let Some(i) = self.queue_index else { return };
        let Some(t) = self.queue.get(i).cloned() else {
            return;
        };

        let path_str = t.path.to_string_lossy();
        if path_str.starts_with("spotify:track:") {
            // Route to Spotify Connect
            let uri = path_str.to_string();
            if let Some(api) = self.spotify.as_mut() {
                match api.play_uri(&uri) {
                    Ok(_) => {
                        self.set_info(format!("Spotify ▶ {}", t.display()));
                        self.push_history(t);
                    }
                    Err(e) => self.set_info(format!("Spotify play error: {e}")),
                }
            } else {
                self.set_error("Spotify: not authorized — press Shift+P to login");
            }
            return;
        }

        // Apply ReplayGain scaling
        self.player.rg_scale = rg_scale(&t, self.replaygain_mode);

        // Local file or HTTP/YouTube stream — build the source off the UI thread so
        // yt-dlp spawn / HTTP connect / symphonia probe don't freeze input (issue #58).
        let stream_err = self.player.stream_err_handle();
        let stream_title = self.player.stream_title_handle();
        let (tx, rx) = std::sync::mpsc::channel();
        let t_clone = t.clone();
        std::thread::spawn(move || {
            let result =
                crate::audio::build_source(&t_clone, std::time::Duration::ZERO, stream_err, stream_title)
                    .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.load_rx = Some(rx);
        self.loading_track = Some(t.clone());
        self.set_info(format!("Loading: {}…", t.display()));
    }

    /// Returns true if the given track is an HTTP/YouTube stream — for streams we
    /// must rebuild the source (yt-dlp respawn) and therefore want to go through the
    /// background loader so the UI does not freeze (issue #57).
    fn track_is_stream(t: &Track) -> bool {
        let p = t.path.to_string_lossy();
        p.starts_with("http://") || p.starts_with("https://")
    }

    fn seek_relative_async(&mut self, delta_secs: i64) {
        let Some(track) = self.player.current().cloned() else {
            return;
        };
        if !Self::track_is_stream(&track) {
            if let Err(e) = self.player.seek_relative(delta_secs) {
                self.set_info(format!("Seek error: {e}"));
            }
            return;
        }
        let cur_ms = self.player.elapsed().as_millis() as i64;
        let mut new_ms = cur_ms + delta_secs * 1000;
        if new_ms < 0 {
            new_ms = 0;
        }
        if let Some(total) = track.duration {
            let max_ms = total.as_millis().saturating_sub(500) as i64;
            if new_ms > max_ms {
                new_ms = max_ms;
            }
        }
        self.spawn_seek_load(track, Duration::from_millis(new_ms as u64));
    }

    fn seek_fraction_async(&mut self, frac: f32) -> Result<()> {
        let Some(track) = self.player.current().cloned() else {
            return Ok(());
        };
        if !Self::track_is_stream(&track) {
            return self.player.seek_absolute_fraction(frac);
        }
        let Some(total) = track.duration else {
            return Ok(());
        };
        let target_ms = (total.as_millis() as f32 * frac.clamp(0.0, 1.0)) as u64;
        self.spawn_seek_load(track, Duration::from_millis(target_ms));
        Ok(())
    }

    fn spawn_seek_load(&mut self, track: Track, offset: Duration) {
        let stream_err = self.player.stream_err_handle();
        let stream_title = self.player.stream_title_handle();
        let (tx, rx) = std::sync::mpsc::channel();
        let t_clone = track.clone();
        std::thread::spawn(move || {
            let result =
                crate::audio::build_source(&t_clone, offset, stream_err, stream_title)
                    .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.load_rx = Some(rx);
        self.loading_track = Some(track);
        self.pending_seek_offset = Some(offset);
        self.set_info("Seeking…");
    }

    fn handle_media_event(&mut self, ev: souvlaki::MediaControlEvent) {
        use souvlaki::MediaControlEvent as E;
        match ev {
            E::Play if self.player.is_paused() => {
                self.player.toggle();
            }
            E::Pause if !self.player.is_paused() => {
                self.player.toggle();
            }
            E::Toggle => self.player.toggle(),
            E::Next => self.next(),
            E::Previous => self.prev(),
            E::Stop => self.player.stop(),
            _ => {}
        }
    }

    fn spawn_lyrics_fetch(&mut self, t: &Track) {
        let artist = t.artist.clone().unwrap_or_default();
        let title = t.title.clone();
        let album = t.album.clone();
        let duration = t.duration;
        let path = t.path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.lyrics_rx = Some(rx);
        std::thread::spawn(move || {
            let result =
                crate::lyrics::Lyrics::fetch_lrclib(&artist, &title, album.as_deref(), duration);
            let _ = tx.send((path, result));
        });
    }

    fn on_track_started(&mut self, t: Track) {
        let new_art =
            crate::metadata::probe_picture(&t.path).and_then(|bytes| self.art_picker.load(&bytes));
        self.album_art = new_art;
        self.art_generation = self.art_generation.wrapping_add(1);
        self.art_picker.invalidate();
        // #105: if no embedded art and the track carries a remote cover URL
        // (typical for YouTube streams), fetch it off the UI thread. Result is
        // drained in `tick` and applied only if we are still on this track.
        if self.album_art.is_none() {
            if let Some(url) = t.cover_url.clone() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.art_rx = Some(rx);
                let path = t.path.clone();
                std::thread::spawn(move || {
                    let bytes = crate::metadata::fetch_remote_picture(&url);
                    let _ = tx.send((path, bytes));
                });
            } else {
                self.art_rx = None;
            }
        } else {
            self.art_rx = None;
        }
        self.set_info(format!("Playing: {}", t.display()));
        if let Some(s) = &mut self.media_session {
            s.update_metadata(
                &t.title,
                t.artist.as_deref().unwrap_or(""),
                t.album.as_deref(),
                t.duration,
            );
            s.update_playback(true, Duration::ZERO);
        }
        self.lyrics = crate::lyrics::Lyrics::for_track(&t.path);
        // #62: if no local .lrc was found, ask LRCLIB asynchronously. The result is
        // delivered via lyrics_rx so the UI does not block on the HTTP call.
        if self.lyrics.is_none() {
            self.spawn_lyrics_fetch(&t);
        }
        let artist = t.artist.clone().unwrap_or_default();
        let title = t.title.clone();
        let ts = crate::lastfm::now_unix();
        self.lastfm_scrobble_info = Some((artist.clone(), title.clone(), ts));
        if let Some(lfm) = self.lastfm.clone() {
            let a = artist.clone();
            let ti = title.clone();
            std::thread::spawn(move || {
                let _ = lfm.update_now_playing(&a, &ti);
            });
        }
        if let Some(tx) = &self.discord_tx {
            let _ = tx.send(crate::discord::Cmd::Update {
                title: title.clone(),
                artist: artist.clone(),
                start_secs: ts as i64,
            });
        }
        self.push_history(t);
        self.update_prefetch_slots();
    }

    fn update_prefetch_slots(&mut self) {
        let Some(cur_idx) = self.queue_index else {
            self.prefetch.invalidate();
            return;
        };
        if self.queue.is_empty() {
            self.prefetch.invalidate();
            return;
        }

        let next_idx = self.pick_next_index(cur_idx);
        let prev_idx = if cur_idx == 0 {
            if self.queue.len() > 1 {
                Some(self.queue.len() - 1)
            } else {
                None
            }
        } else {
            Some(cur_idx - 1)
        };

        // Prepare Next track slot
        if let Some(n_idx) = next_idx {
            if let Some(target) = self.queue.get(n_idx).cloned() {
                if !Self::track_is_stream(&target) {
                    let path = target.path.clone();
                    let already_ready = self.prefetch.next.as_ref().map(|p| &p.path) == Some(&path);
                    let already_building = self.prefetch.building_next.as_ref() == Some(&path);
                    if !already_ready && !already_building {
                        self.prefetch.next = None;
                        self.prefetch.building_next = Some(path.clone());
                        if let Some(tx) = &self.prefetch.tx {
                            let tx = tx.clone();
                            let stream_err = self.player.stream_err_handle();
                            let stream_title = self.player.stream_title_handle();
                            std::thread::spawn(move || {
                                let res = crate::audio::build_source(&target, Duration::ZERO, stream_err, stream_title)
                                    .map_err(|e| e.to_string());
                                let _ = tx.send((SlotKind::Next, path, res));
                            });
                        }
                    }
                } else {
                    self.prefetch.next = None;
                    self.prefetch.building_next = None;
                }
            }
        } else {
            self.prefetch.next = None;
            self.prefetch.building_next = None;
        }

        // Prepare Prev track slot
        if let Some(p_idx) = prev_idx {
            if let Some(target) = self.queue.get(p_idx).cloned() {
                if !Self::track_is_stream(&target) {
                    let path = target.path.clone();
                    let already_ready = self.prefetch.prev.as_ref().map(|p| &p.path) == Some(&path);
                    let already_building = self.prefetch.building_prev.as_ref() == Some(&path);
                    if !already_ready && !already_building {
                        self.prefetch.prev = None;
                        self.prefetch.building_prev = Some(path.clone());
                        if let Some(tx) = &self.prefetch.tx {
                            let tx = tx.clone();
                            let stream_err = self.player.stream_err_handle();
                            let stream_title = self.player.stream_title_handle();
                            std::thread::spawn(move || {
                                let res = crate::audio::build_source(&target, Duration::ZERO, stream_err, stream_title)
                                    .map_err(|e| e.to_string());
                                let _ = tx.send((SlotKind::Prev, path, res));
                            });
                        }
                    }
                } else {
                    self.prefetch.prev = None;
                    self.prefetch.building_prev = None;
                }
            }
        } else {
            self.prefetch.prev = None;
            self.prefetch.building_prev = None;
        }
    }

    fn pick_next_index(&self, current: usize) -> Option<usize> {
        if self.queue.is_empty() {
            return None;
        }
        if self.shuffle && self.queue.len() > 1 {
            let mut idx = pseudo_random(self.queue.len());
            if idx == current {
                idx = (idx + 1) % self.queue.len();
            }
            Some(idx)
        } else if current + 1 < self.queue.len() {
            Some(current + 1)
        } else if matches!(self.repeat, RepeatMode::All) {
            Some(0)
        } else {
            None
        }
    }

    fn next(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let i = self.queue_index.unwrap_or(0);
        let new = self.pick_next_index(i).unwrap_or(0);
        self.queue_index = Some(new);
        self.queue_state.select(Some(new));

        if let Some(track) = self.queue.get(new).cloned() {
            if let Some(preloaded) = self.prefetch.next.take() {
                if preloaded.path == track.path {
                    self.play_instant(preloaded.source, track);
                    return;
                }
            }
        }
        self.play_current();
    }

    fn prev(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let i = self.queue_index.unwrap_or(0);
        let new = if i == 0 { self.queue.len().saturating_sub(1) } else { i - 1 };
        self.queue_index = Some(new);
        self.queue_state.select(Some(new));

        if let Some(track) = self.queue.get(new).cloned() {
            if let Some(preloaded) = self.prefetch.prev.take() {
                if preloaded.path == track.path {
                    self.play_instant(preloaded.source, track);
                    return;
                }
            }
        }
        self.play_current();
    }

    fn advance(&mut self) {
        if matches!(self.repeat, RepeatMode::One) {
            self.play_current();
            return;
        }
        if let Some(i) = self.queue_index {
            if let Some(new) = self.pick_next_index(i) {
                self.queue_index = Some(new);
                self.queue_state.select(Some(new));
                if let Some(track) = self.queue.get(new).cloned() {
                    if let Some(preloaded) = self.prefetch.next.take() {
                        if preloaded.path == track.path {
                            self.play_instant(preloaded.source, track);
                            return;
                        }
                    }
                }
                self.play_current();
            } else {
                self.player.stop();
                self.queue_index = None;
                self.prefetch.invalidate();
            }
        }
    }

    fn save_playlist_named(&mut self, name: String) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.set_info(format!("Playlist dir error: {e}"));
                return;
            }
        };
        if std::fs::create_dir_all(&dir).is_err() {
            self.set_info(format!("Could not create {}", dir.display()));
            return;
        }
        let safe_name = if name.is_empty() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("queue-{stamp}")
        } else {
            name.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        let path = dir.join(format!("{safe_name}.m3u"));
        let mut text = String::from("#EXTM3U\n");
        for t in &self.queue {
            // #119: persist title/artist/duration via the standard `#EXTINF`
            // directive so reloading a playlist in another session shows real
            // track names instead of raw stream URLs.
            let secs = t.duration.map(|d| d.as_secs() as i64).unwrap_or(-1);
            let display = match &t.artist {
                Some(a) if !a.is_empty() => format!("{a} - {}", t.title),
                _ => t.title.clone(),
            };
            // EXTINF must be a single line; strip any embedded newlines.
            let display = display.replace(['\r', '\n'], " ");
            text.push_str(&format!("#EXTINF:{secs},{display}\n"));
            text.push_str(&t.path.display().to_string());
            text.push('\n');
        }
        match std::fs::write(&path, text) {
            Ok(_) => {
                self.active_playlist_name = Some(safe_name.clone());
                self.set_info(format!("Saved: {safe_name}.m3u"));
            }
            Err(e) => self.set_info(format!("Save error: {e}")),
        }
    }

    fn open_playlist_browser(&mut self) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.set_info(format!("Playlist dir error: {e}"));
                return;
            }
        };
        let mut entries: Vec<PlaylistEntry> = std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()).map(|ext| {
                    ext.eq_ignore_ascii_case("m3u") || ext.eq_ignore_ascii_case("m3u8")
                }).unwrap_or(false)
            })
            .filter_map(|e| {
                let path = e.path();
                let name = path.file_stem()?.to_str()?.to_string();
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let count = text
                    .lines()
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .count();
                Some(PlaylistEntry {
                    name,
                    path,
                    track_count: count,
                })
            })
            .collect();
        // #84: sort by last-played desc when we have history; ties and entries
        // never played fall back to alphabetical so the UI stays predictable.
        let history = &self.play_history;
        entries.sort_by(|a, b| {
            let la = history
                .playlist_record(&crate::history::PlaylistRef::Local {
                    path: a.path.clone(),
                })
                .map(|r| r.last_played)
                .unwrap_or(0);
            let lb = history
                .playlist_record(&crate::history::PlaylistRef::Local {
                    path: b.path.clone(),
                })
                .map(|r| r.last_played)
                .unwrap_or(0);
            lb.cmp(&la).then_with(|| a.name.cmp(&b.name))
        });
        if entries.is_empty() {
            self.set_info("No playlists saved yet.");
            return;
        }
        self.playlist_browser_entries = entries;
        self.playlist_browser_row = 0;
        self.playlist_browser_delete_confirm = None;
        self.show_playlist_browser = true;
    }

    fn handle_playlist_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_playlist_browser = false;
                self.playlist_browser_delete_confirm = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.playlist_browser_row = self.playlist_browser_row.saturating_sub(1);
                self.playlist_browser_delete_confirm = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.playlist_browser_entries.len().saturating_sub(1);
                if self.playlist_browser_row < max {
                    self.playlist_browser_row += 1;
                }
                self.playlist_browser_delete_confirm = None;
            }
            KeyCode::Enter => {
                self.load_playlist_at_row(false);
            }
            KeyCode::Char('a') => {
                self.load_playlist_at_row(true);
            }
            KeyCode::Char('D') => {
                let row = self.playlist_browser_row;
                if self.playlist_browser_delete_confirm == Some(row) {
                    if let Some(entry) = self.playlist_browser_entries.get(row).cloned() {
                        if std::fs::remove_file(&entry.path).is_ok() {
                            self.playlist_browser_entries.remove(row);
                            self.playlist_browser_row =
                                row.min(self.playlist_browser_entries.len().saturating_sub(1));
                            self.set_info(format!("Deleted: {}", entry.name));
                            if self.active_playlist_name.as_deref() == Some(&entry.name) {
                                self.active_playlist_name = None;
                            }
                        }
                    }
                    self.playlist_browser_delete_confirm = None;
                    if self.playlist_browser_entries.is_empty() {
                        self.show_playlist_browser = false;
                    }
                } else {
                    self.playlist_browser_delete_confirm = Some(row);
                    self.set_info("Press Shift+D again to confirm deletion.");
                }
            }
            _ => {}
        }
    }

    fn load_playlist_at_row(&mut self, append: bool) {
        let Some(entry) = self
            .playlist_browser_entries
            .get(self.playlist_browser_row)
            .cloned()
        else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&entry.path) else {
            self.set_info(format!("Could not read {}", entry.path.display()));
            return;
        };
        // Snapshot before any mutation so undo restores the exact pre-load state
        self.push_undo_snapshot(format!(
            "{} playlist '{}'",
            if append { "appended" } else { "loaded" },
            entry.name
        ));
        if !append {
            self.queue.clear();
            self.queue_state.select(None);
            self.queue_index = None;
        }
        let start = self.queue.len();
        // #119: track the most recent `#EXTINF` so the following path/URL
        // inherits its title/artist/duration. Cleared after consumption so a
        // stray EXTINF doesn't bleed onto an unrelated entry.
        let mut pending_extinf: Option<(Option<std::time::Duration>, Option<String>, String)> =
            None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("#EXTINF:") {
                pending_extinf = Some(parse_extinf(rest));
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            let (dur, artist, title) = match pending_extinf.take() {
                Some((d, a, t)) => (d, a, Some(t)),
                None => (None, None, None),
            };
            if line.starts_with("http://") || line.starts_with("https://") {
                self.queue.push(Track::from_url_with_meta(
                    line.to_string(),
                    title,
                    artist,
                    dur,
                ));
            } else {
                let candidate = std::path::Path::new(line);
                let p = if candidate.is_relative() {
                    entry.path.parent().map(|dir| dir.join(candidate)).unwrap_or_else(|| std::path::PathBuf::from(line))
                } else {
                    std::path::PathBuf::from(line)
                };
                if p.exists() {
                    self.queue.push(Track::from_path_with_meta(p));
                }
            }
        }
        let loaded = self.queue.len() - start;
        if loaded == 0 {
            // Nothing added — discard the snapshot we just pushed
            self.undo_stack.pop_back();
        }
        if !append {
            if !self.queue.is_empty() {
                self.queue_state.select(Some(0));
            }
            self.active_playlist_name = Some(entry.name.clone());
        }
        // #84: record this playback in the playlist history. Only counts when
        // tracks actually loaded so an empty/broken `.m3u` doesn't pollute the
        // recent list.
        if loaded > 0 {
            self.play_history.record_playlist_play(
                crate::history::PlaylistRef::Local {
                    path: entry.path.clone(),
                },
                entry.name.clone(),
                None,
                loaded as u32,
            );
        }
        self.show_playlist_browser = false;
        self.set_info(if append {
            format!("Appended {} tracks from '{}'", loaded, entry.name)
        } else {
            format!("Loaded {} tracks from '{}'", loaded, entry.name)
        });
    }

    fn save_eq_preset(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        let snap = self.player.eq().snapshot();
        let preset = crate::config::EqPreset {
            name: name.clone(),
            low_db: snap.low_db,
            mid_db: snap.mid_db,
            high_db: snap.high_db,
        };
        // Replace if name already exists, otherwise append
        if let Some(existing) = self.custom_eq_presets.iter_mut().find(|p| p.name == name) {
            *existing = preset;
        } else {
            self.custom_eq_presets.push(preset);
        }
        let store = crate::config::EqPresets {
            presets: self.custom_eq_presets.clone(),
        };
        match store.save() {
            Ok(_) => self.set_info(format!("EQ preset '{name}' saved.")),
            Err(e) => self.set_info(format!("EQ preset save error: {e}")),
        }
    }

    fn save_profile(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        let snap = self.player.eq().snapshot();
        let profile = crate::config::Profile {
            name: name.clone(),
            theme: self.config.theme.clone(),
            volume: self.player.volume(),
            shuffle: self.shuffle,
            repeat: matches!(self.repeat, RepeatMode::All | RepeatMode::One),
            eq_low_db: snap.low_db,
            eq_mid_db: snap.mid_db,
            eq_high_db: snap.high_db,
        };
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.name == name) {
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
        let store = crate::config::Profiles {
            profiles: self.profiles.clone(),
        };
        match store.save() {
            Ok(_) => self.set_info(format!("Profile '{name}' saved.")),
            Err(e) => self.set_info(format!("Profile save error: {e}")),
        }
    }

    fn spotify_search(&mut self) {
        let query = self.spotify_browser_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let Some(api) = self.spotify.clone() else {
            return;
        };
        self.set_info(format!("Searching Spotify: \"{query}\"…"));
        self.spotify_browser_results.clear();
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<Track>, String>>();
        self.spotify_search_rx = Some(rx);
        std::thread::spawn(move || {
            let mut api = api;
            let r = api.search(&query, 30).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
    }

    fn spotify_load_my_playlists(&mut self) {
        let Some(mut api) = self.spotify.clone() else {
            return;
        };
        match api.my_playlists() {
            Ok(playlists) => {
                self.spotify_my_playlists = playlists;
                if !self.spotify_my_playlists.is_empty() {
                    self.spotify_playlist_row = 0;
                }
            }
            Err(e) => self.set_info(format!("Spotify playlists error: {e}")),
        }
    }

    fn spotify_load_liked(&mut self) {
        let Some(api) = self.spotify.clone() else {
            return;
        };
        self.set_info("Loading liked songs…");
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);
        std::thread::spawn(move || {
            let mut api = api;
            let r = api.liked_tracks(50).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
    }

    fn spotify_load_playlist(&mut self, id: String, name: String) {
        let Some(api) = self.spotify.clone() else {
            return;
        };
        self.set_info(format!("Loading playlist \"{name}\"…"));
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);
        std::thread::spawn(move || {
            let mut api = api;
            let r = api.playlist_tracks(&id).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
    }

    fn handle_spotify_browser_key(&mut self, key: KeyEvent) {
        // While typing a search query
        if self.spotify_browser_query_editing {
            match key.code {
                KeyCode::Esc => {
                    self.spotify_browser_query_editing = false;
                    if self.spotify_browser_query.is_empty() {
                        self.show_spotify_browser = false;
                    }
                }
                KeyCode::Enter => {
                    self.spotify_browser_query_editing = false;
                    self.spotify_browser_tab = SpotifyTab::Search;
                    self.spotify_search();
                }
                KeyCode::Backspace => {
                    self.spotify_browser_query.pop();
                }
                KeyCode::Char(c) => {
                    self.spotify_browser_query.push(c);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.show_spotify_browser = false;
            }
            // Tab cycle: Search → MyPlaylists → LikedSongs
            KeyCode::Tab => {
                self.spotify_browser_tab = match self.spotify_browser_tab {
                    SpotifyTab::Search => {
                        if self.spotify_my_playlists.is_empty() {
                            self.spotify_load_my_playlists();
                        }
                        SpotifyTab::MyPlaylists
                    }
                    SpotifyTab::MyPlaylists => {
                        self.spotify_load_liked();
                        SpotifyTab::LikedSongs
                    }
                    SpotifyTab::LikedSongs => SpotifyTab::Search,
                };
                self.spotify_browser_row = 0;
                self.spotify_playlist_row = 0;
            }
            KeyCode::Char('/') | KeyCode::Char('s') => {
                self.spotify_browser_tab = SpotifyTab::Search;
                self.spotify_browser_query_editing = true;
            }
            KeyCode::Up | KeyCode::Char('k') => match self.spotify_browser_tab {
                SpotifyTab::MyPlaylists => {
                    if self.spotify_playlist_row > 0 {
                        self.spotify_playlist_row -= 1;
                    }
                }
                _ => {
                    if self.spotify_browser_row > 0 {
                        self.spotify_browser_row -= 1;
                    }
                }
            },
            KeyCode::Down | KeyCode::Char('j') => match self.spotify_browser_tab {
                SpotifyTab::MyPlaylists => {
                    if self.spotify_playlist_row + 1 < self.spotify_my_playlists.len() {
                        self.spotify_playlist_row += 1;
                    }
                }
                _ => {
                    if self.spotify_browser_row + 1 < self.spotify_browser_results.len() {
                        self.spotify_browser_row += 1;
                    }
                }
            },
            // Enter: play immediately
            KeyCode::Enter => {
                self.show_spotify_browser = false;
                match self.spotify_browser_tab {
                    SpotifyTab::MyPlaylists => {
                        if let Some((id, name, _)) = self
                            .spotify_my_playlists
                            .get(self.spotify_playlist_row)
                            .cloned()
                        {
                            self.spotify_load_playlist(id, name);
                        }
                    }
                    _ => {
                        if let Some(t) = self
                            .spotify_browser_results
                            .get(self.spotify_browser_row)
                            .cloned()
                        {
                            self.queue.push(t.clone());
                            self.queue_index = Some(self.queue.len() - 1);
                            self.queue_state.select(self.queue_index);
                            self.play_current();
                        }
                    }
                }
            }
            // 'a': enqueue without playing
            KeyCode::Char('a') => match self.spotify_browser_tab {
                SpotifyTab::MyPlaylists => {
                    if let Some((id, name, _)) = self
                        .spotify_my_playlists
                        .get(self.spotify_playlist_row)
                        .cloned()
                    {
                        self.show_spotify_browser = false;
                        self.spotify_load_playlist(id, name);
                    }
                }
                _ => {
                    if let Some(t) = self
                        .spotify_browser_results
                        .get(self.spotify_browser_row)
                        .cloned()
                    {
                        self.queue.push(t);
                        self.set_info("Added to queue.");
                    }
                }
            },
            _ => {}
        }
    }

    fn apply_profile(&mut self, idx: usize) {
        let Some(p) = self.profiles.get(idx).cloned() else {
            return;
        };
        self.player.set_volume(p.volume);
        self.config.playback.default_volume = p.volume;
        self.shuffle = p.shuffle;
        self.repeat = if p.repeat {
            RepeatMode::All
        } else {
            RepeatMode::Off
        };
        self.player.eq().set(crate::eq::EqState {
            low_db: p.eq_low_db,
            mid_db: p.eq_mid_db,
            high_db: p.eq_high_db,
        });
        if self.config.theme != p.theme {
            if let Ok(theme) = crate::theme::Theme::load(&p.theme) {
                self.config.theme = p.theme.clone();
                self.theme = theme;
            }
        }
        let _ = self.config.save();
        self.set_info(format!("Profile '{}' loaded.", p.name));
    }

    fn handle_profile_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.show_profile_browser = false;
            }
            KeyCode::Up | KeyCode::Char('k') if self.profile_browser_row > 0 => {
                self.profile_browser_row -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.profile_browser_row + 1 < self.profiles.len() =>
            {
                self.profile_browser_row += 1;
            }
            KeyCode::Enter => {
                let row = self.profile_browser_row;
                self.show_profile_browser = false;
                self.apply_profile(row);
            }
            KeyCode::Char('n') => {
                self.show_profile_browser = false;
                self.profile_name_editing = true;
                self.profile_name_input.clear();
                self.set_info("Profile name (Enter to save, Esc to cancel):");
            }
            KeyCode::Char('D') if self.profile_browser_row < self.profiles.len() => {
                let removed = self.profiles.remove(self.profile_browser_row);
                if self.profile_browser_row > 0 {
                    self.profile_browser_row -= 1;
                }
                let store = crate::config::Profiles {
                    profiles: self.profiles.clone(),
                };
                let _ = store.save();
                self.set_info(format!("Profile '{}' deleted.", removed.name));
            }
            _ => {}
        }
    }

    /// Render rich-protocol album art (Kitty / iTerm2) after ratatui's frame draw.
    /// Called every frame when art is loaded; a no-op for block mode (handled in ui.rs).
    fn render_overlay_art(&mut self) {
        let Some(img) = self.album_art.as_ref() else {
            return;
        };
        let area = self.layout.art_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        let protocol = self.art_picker.protocol;
        if matches!(protocol, crate::album_art::Protocol::Blocks) {
            // Handled inside ratatui render loop (ui.rs); nothing to do here.
            return;
        }
        // #91: cached escape sequence keyed by (track change, area, protocol).
        // Subsequent frames with the same art+geometry skip resize + base64 encode.
        let key = crate::album_art::ArtCacheKey {
            generation: self.art_generation,
            protocol,
            x: area.x,
            y: area.y,
            w: area.width,
            h: area.height,
        };
        let bytes = self.art_picker.cached_overlay(key, || match protocol {
            crate::album_art::Protocol::Kitty => crate::album_art::build_kitty(img, area, 8, 16),
            crate::album_art::Protocol::Iterm2 => crate::album_art::build_iterm2(img, area),
            crate::album_art::Protocol::Blocks => Vec::new(),
        });
        let mut out = std::io::stdout().lock();
        let _ = std::io::Write::write_all(&mut out, bytes);
        let _ = std::io::Write::flush(&mut out);
    }
}

/// Parse the payload of an M3U `#EXTINF:` line (everything after the colon).
///
/// Expected shape: `<seconds>,<display>` where `<display>` is conventionally
/// `Artist - Title` but may be just the title. Negative seconds (commonly
/// `-1`) mean "unknown duration".
fn parse_extinf(rest: &str) -> (Option<std::time::Duration>, Option<String>, String) {
    let (secs_str, display) = match rest.split_once(',') {
        Some((s, d)) => (s.trim(), d.trim()),
        None => ("", rest.trim()),
    };
    let duration = secs_str
        .parse::<i64>()
        .ok()
        .filter(|s| *s >= 0)
        .map(|s| std::time::Duration::from_secs(s as u64));
    let (artist, title) = match display.split_once(" - ") {
        Some((a, t)) if !a.is_empty() && !t.is_empty() => (Some(a.to_string()), t.to_string()),
        _ => (None, display.to_string()),
    };
    (duration, artist, title)
}

#[cfg(test)]
mod extinf_tests {
    use super::parse_extinf;
    use std::time::Duration;

    #[test]
    fn parses_artist_title_and_duration() {
        let (d, a, t) = parse_extinf("213,Some Artist - Some Title");
        assert_eq!(d, Some(Duration::from_secs(213)));
        assert_eq!(a.as_deref(), Some("Some Artist"));
        assert_eq!(t, "Some Title");
    }

    #[test]
    fn unknown_duration_returns_none() {
        let (d, _, _) = parse_extinf("-1,X - Y");
        assert!(d.is_none());
    }

    #[test]
    fn missing_artist_falls_back_to_title_only() {
        let (_, a, t) = parse_extinf("0,Just a title");
        assert!(a.is_none());
        assert_eq!(t, "Just a title");
    }

    #[test]
    fn handles_payload_without_comma() {
        let (d, a, t) = parse_extinf("only title");
        assert!(d.is_none());
        assert!(a.is_none());
        assert_eq!(t, "only title");
    }
}
