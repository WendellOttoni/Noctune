mod input;
mod library;
mod playback;
mod prefetch;
mod scan;
mod services;
pub mod types;
mod util;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use image::DynamicImage;
use ratatui::widgets::ListState;
use std::{path::PathBuf, time::Duration};

use notify::Watcher as _;

// Re-exports of submodules and types
use self::prefetch::{PrefetchSlots, PreloadedTrack, SlotKind};
use self::scan::scan_library_with_progress;
pub use self::types::*;
use self::util::{rg_scale, sort_tracks};

use crate::{
    album_art::ArtPicker,
    audio::{CrossfadeStatus, Player, Track},
    cache::{cache_path, MetadataCache},
    config::Config,
    keybinds::Bindings,
    theme::Theme,
    tui::Tui,
    ui,
    visualizer::VizTap,
};

pub struct App {
    #[allow(dead_code)]
    pub config: Config,
    pub theme: Theme,
    pub player: Player,
    pub tap: VizTap,
    pub library: Vec<Track>,
    pub queue: Vec<Track>,
    pub(crate) undo_stack: std::collections::VecDeque<UndoSnapshot>,
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
    pub endless_mode: bool,
    pub eq_preset_idx: usize,
    pub show_info: bool,
    pub theme_names: Vec<String>,
    pub theme_idx: usize,
    pub last_drag_seek: Option<std::time::Instant>,
    pub clear_confirm_until: Option<std::time::Instant>,
    pub url_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Track>, String>>>,
    pub download_rx: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
    pub load_rx: Option<std::sync::mpsc::Receiver<Result<crate::audio::SymphoniaSource, String>>>,
    pub loading_track: Option<Track>,
    pub pending_seek_offset: Option<Duration>,
    pub prefetch: PrefetchSlots,
    pub scan_rx: Option<std::sync::mpsc::Receiver<Vec<Track>>>,
    pub scan_progress_rx: Option<std::sync::mpsc::Receiver<(usize, usize)>>,
    pub scan_progress: Option<(usize, usize)>,
    pub fs_event_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pub _fs_watcher: Option<notify::RecommendedWatcher>,
    pub config_watcher_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pub _config_watcher: Option<notify::RecommendedWatcher>,
    pub lyrics_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Option<crate::lyrics::Lyrics>)>>,
    pub art_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Option<Vec<u8>>)>>,
    pub library_revision: u64,
    pub history_revision: u64,
    pub play_history_revision: u64,
    pub smart_cache: Option<SmartRowsCache>,
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
    pub ipc_server: Option<crate::ipc::IpcServer>,
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
    pub(crate) current_play_recorded: bool,
    pub lastfm: Option<crate::lastfm::LastfmClient>,
    pub(crate) lastfm_pending_token: Option<String>,
    pub(crate) lastfm_scrobble_info: Option<(String, String, u64)>,
    pub(crate) lastfm_scrobbled: bool,
    pub(crate) discord_tx: Option<std::sync::mpsc::Sender<crate::discord::Cmd>>,
    pub show_device_selector: bool,
    pub device_list: Vec<String>,
    pub device_selector_row: usize,
    pub show_eq_tuner: bool,
    pub eq_tuner_band: usize,
    pub(crate) pending_gapless_idx: Option<usize>,
    pub playlist_name_editing: bool,
    pub playlist_name_input: String,
    pub show_playlist_browser: bool,
    pub playlist_browser_entries: Vec<PlaylistEntry>,
    pub playlist_browser_row: usize,
    pub playlist_browser_delete_confirm: Option<usize>,
    pub active_playlist_name: Option<String>,
    pub album_art: Option<DynamicImage>,
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
    pub(crate) spotify_search_rx: Option<std::sync::mpsc::Receiver<Result<Vec<Track>, String>>>,
    pub show_tag_editor: bool,
    pub tag_editor_path: Option<PathBuf>,
    pub tag_editor_fields: [String; 5],
    pub tag_editor_row: usize,
    pub show_radio_browser: bool,
    pub show_radio_custom_modal: bool,
    pub radio_custom_fields: [String; 3],
    pub radio_custom_field_idx: usize,
    pub radio_tab: crate::radio_browser::RadioTab,
    pub radio_curated_list: Vec<crate::radio_browser::RadioStation>,
    pub radio_search_results: Vec<crate::radio_browser::RadioStation>,
    pub radio_row: usize,
    pub radio_category_idx: usize,
    pub radio_focus_pane: usize,
    pub radio_search_query: String,
    pub radio_search_editing: bool,
    pub radio_search_rx:
        Option<std::sync::mpsc::Receiver<Result<Vec<crate::radio_browser::RadioStation>, String>>>,
    pub radio_history: Vec<(String, Option<String>, u64)>,
    pub update_info: Option<crate::updater::UpdateInfo>,
    pub is_updating: bool,
    pub(crate) update_check_rx:
        Option<std::sync::mpsc::Receiver<Result<Option<crate::updater::UpdateInfo>, String>>>,
    pub(crate) update_apply_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    pub stream_reconnect_attempts: u32,
    pub show_lyrics: bool,
    pub lyrics_scroll: usize,
    pub lyrics_auto_scroll: bool,
    pub show_command_palette: bool,
    pub command_palette_input: String,
    pub command_palette_row: usize,
    pub command_palette_matches: Vec<PaletteItem>,
    pub show_subsonic_browser: bool,
    pub subsonic_browser_tab: SubsonicTab,
    pub subsonic_browser_query: String,
    pub subsonic_browser_query_editing: bool,
    pub subsonic_browser_results: Vec<crate::audio::Track>,
    pub subsonic_browser_albums: Vec<crate::subsonic::SubsonicAlbum>,
    pub subsonic_browser_playlists: Vec<crate::subsonic::SubsonicPlaylist>,
    pub subsonic_browser_row: usize,
    pub(crate) subsonic_rx:
        Option<std::sync::mpsc::Receiver<Result<crate::subsonic::SubsonicFetchResult, String>>>,
    pub plugins: Option<crate::plugin::PluginEngine>,
    pub db: Option<crate::db::LibraryDatabase>,
    pub native_spotify: Option<crate::spotify::NativeSpotifySession>,
    pub show_vault_browser: bool,
    pub vault_query: String,
    pub vault_query_editing: bool,
    pub vault_results: Vec<crate::audio::Track>,
    pub vault_row: usize,
    pub(crate) vault_rx:
        Option<std::sync::mpsc::Receiver<Result<Vec<crate::audio::Track>, String>>>,
    pub show_share_modal: bool,
    pub share_playlist_title: String,
    pub share_playlist_desc: String,
    pub share_playlist_visibility: crate::share::Visibility,
    pub share_playlist_tags: String,
    pub share_modal_field: usize,
    pub show_browse_modal: bool,
    pub browse_search_query: String,
    pub browse_search_editing: bool,
    pub browse_results: Vec<crate::share::api::SharedPlaylistSummary>,
    pub browse_row: usize,
    pub(crate) browse_rx: Option<
        std::sync::mpsc::Receiver<Result<Vec<crate::share::api::SharedPlaylistSummary>, String>>,
    >,
    pub(crate) share_publish_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}

impl App {
    pub fn set_status<S: Into<String>>(&mut self, kind: StatusKind, msg: S) {
        self.status = msg.into();
        self.status_kind = kind;
    }

    pub fn set_info<S: Into<String>>(&mut self, msg: S) {
        self.set_status(StatusKind::Info, msg);
    }

    pub fn set_error<S: Into<String>>(&mut self, msg: S) {
        self.set_status(StatusKind::Error, msg);
    }

    pub fn sleep_remaining(&self) -> Option<Duration> {
        self.sleep_until
            .map(|t| t.saturating_duration_since(std::time::Instant::now()))
    }

    pub(crate) fn toggle_sleep_timer(&mut self) {
        if self.sleep_until.is_some() {
            self.sleep_until = None;
            self.set_info("Sleep timer cancelled.");
        } else {
            let when = std::time::Instant::now() + Duration::from_secs(30 * 60);
            self.sleep_until = Some(when);
            self.set_info("Sleep timer: 30 min.");
        }
    }

    pub(crate) fn push_history(&mut self, t: Track) {
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

    pub fn new(config: Config, theme: Theme, art_picker: ArtPicker) -> Result<Self> {
        crate::ytdlp::configure_retries(config.ytdlp.clone());
        let history_cfg = config.history.clone();
        let mut player = Player::new(
            config.playback.default_volume,
            config.visualizer.sensitivity,
        )?;
        player.crossfade_secs = config.playback.crossfade_secs;
        let tap = player.tap();

        let config_shuffle = config.playback.shuffle;
        let config_repeat = config.playback.repeat;
        let config_endless = config.playback.endless_mode;
        let config_keybinds = config.keybinds.clone();
        let spotify_client_id = config.spotify.client_id.clone();
        let spotify_redirect_uri = config.spotify.redirect_uri();
        let spotify_port = config.spotify.redirect_port;
        let _ = spotify_port;

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

        let spotify = crate::spotify::load_tokens()
            .filter(|_| !spotify_client_id.is_empty())
            .and_then(|t| crate::spotify::SpotifyApi::new(spotify_client_id.clone(), t).ok());

        let (scan_tx, scan_rx) = std::sync::mpsc::channel::<Vec<Track>>();
        let (progress_tx, scan_progress_rx) = std::sync::mpsc::channel::<(usize, usize)>();
        let scan_dirs = config.music_dirs.clone();
        let cache_cfg = config.cache.clone();
        std::thread::spawn(move || {
            let cache_file = cache_path();
            let mut cache = cache_file
                .as_deref()
                .map(MetadataCache::load)
                .unwrap_or_default();
            cache.prune(cache_cfg.expire_days, cache_cfg.max_size_mb);
            let tracks = scan_library_with_progress(&scan_dirs, &mut cache, Some(progress_tx));
            if let Some(p) = &cache_file {
                cache.save(p);
            }
            let _ = scan_tx.send(tracks);
        });

        let (fs_tx, fs_event_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let _fs_watcher = if config.library.watch_for_changes {
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
            endless_mode: config_endless,
            eq_preset_idx: 0,
            show_info: false,
            theme_names: Vec::new(),
            theme_idx: 0,
            last_drag_seek: None,
            clear_confirm_until: None,
            url_rx: None,
            download_rx: None,
            load_rx: None,
            loading_track: None,
            pending_seek_offset: None,
            prefetch: PrefetchSlots::new(),
            scan_rx: Some(scan_rx),
            scan_progress_rx: Some(scan_progress_rx),
            scan_progress: None,
            fs_event_rx: Some(fs_event_rx),
            _fs_watcher,
            config_watcher_rx: None,
            _config_watcher: None,
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
            ipc_server: crate::ipc::IpcServer::start(),
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
            show_radio_custom_modal: false,
            radio_custom_fields: [String::new(), String::new(), String::new()],
            radio_custom_field_idx: 0,
            radio_tab: crate::radio_browser::RadioTab::Curated,
            radio_curated_list: crate::radio_browser::all_stations(),
            radio_search_results: Vec::new(),
            radio_row: 0,
            radio_category_idx: 0,
            radio_focus_pane: 0,
            radio_search_query: String::new(),
            radio_search_editing: false,
            radio_search_rx: None,
            radio_history: Vec::new(),
            update_info: None,
            is_updating: false,
            update_check_rx: None,
            update_apply_rx: None,
            stream_reconnect_attempts: 0,
            show_lyrics: false,
            lyrics_scroll: 0,
            lyrics_auto_scroll: true,
            show_command_palette: false,
            command_palette_input: String::new(),
            command_palette_row: 0,
            command_palette_matches: Vec::new(),
            show_subsonic_browser: false,
            subsonic_browser_tab: SubsonicTab::Search,
            subsonic_browser_query: String::new(),
            subsonic_browser_query_editing: false,
            subsonic_browser_results: Vec::new(),
            subsonic_browser_albums: Vec::new(),
            subsonic_browser_playlists: Vec::new(),
            subsonic_browser_row: 0,
            subsonic_rx: None,
            plugins: None,
            db: None,
            native_spotify: None,
            show_vault_browser: false,
            vault_query: String::new(),
            vault_query_editing: false,
            vault_results: Vec::new(),
            vault_row: 0,
            vault_rx: None,
            show_share_modal: false,
            share_playlist_title: String::new(),
            share_playlist_desc: String::new(),
            share_playlist_visibility: crate::share::Visibility::Public,
            share_playlist_tags: String::new(),
            share_modal_field: 0,
            show_browse_modal: false,
            browse_search_query: String::new(),
            browse_search_editing: false,
            browse_results: Vec::new(),
            browse_row: 0,
            browse_rx: None,
            share_publish_rx: None,
        };

        if let Some(tokens) = crate::spotify::load_tokens() {
            let mut native_sess = crate::spotify::NativeSpotifySession::new("Noctune".to_string());
            let _ = native_sess.start(&tokens.access_token);
            app.native_spotify = Some(native_sess);
        }

        if let Ok(p) = crate::config::db_path() {
            if let Ok(database) = crate::db::LibraryDatabase::open(&p) {
                app.db = Some(database);
            }
        }

        if let Ok(mut engine) = crate::plugin::PluginEngine::new() {
            if let Ok(p_dir) = crate::config::plugins_dir() {
                let loaded = engine.load_plugins_dir(&p_dir);
                if !loaded.is_empty() {
                    tracing::info!(target: "plugins", "Loaded {} Lua plugin(s): {:?}", loaded.len(), loaded);
                }
            }
            app.plugins = Some(engine);
        }

        let (update_tx, update_rx) = std::sync::mpsc::channel();
        app.update_check_rx = Some(update_rx);
        std::thread::spawn(move || {
            let res = crate::updater::check_for_updates().map_err(|e| e.to_string());
            let _ = update_tx.send(res);
        });

        app.rearm_config_watcher();

        if let Some(pos) = crate::eq::PRESETS
            .iter()
            .position(|(name, _)| *name == app.config.playback.eq_preset)
        {
            app.eq_preset_idx = pos;
            app.player.eq().set(crate::eq::PRESETS[pos].1);
        }

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
        self.config.playback.default_volume = self.player.volume();
        self.config.playback.shuffle = self.shuffle;
        self.config.playback.repeat = matches!(self.repeat, RepeatMode::All | RepeatMode::One);
        let _ = self.config.save();
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        self.tick_count = self.tick_count.wrapping_add(1);

        crate::media_session::pump_messages();

        let (p_msgs, p_acts) = if let Some(engine) = &self.plugins {
            engine.set_state(self.player.current(), self.player.volume());
            (engine.drain_messages(), engine.drain_actions())
        } else {
            (Vec::new(), Vec::new())
        };
        for msg in p_msgs {
            self.set_info(msg);
        }
        for act in p_acts {
            self.run_action(act);
        }

        if self.tick_count.is_multiple_of(30) {
            self.sys_stats.refresh();
        }

        if let Some(rx) = &self.scan_progress_rx {
            while let Ok(p) = rx.try_recv() {
                self.scan_progress = Some(p);
            }
        }

        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(mut tracks) => {
                    sort_tracks(&mut tracks, self.sort);
                    let prev_n = self.library.len();
                    let n = tracks.len();
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
                    if let Some(db) = &self.db {
                        let tracks_clone = self.library.clone();
                        let db_clone = db.clone();
                        std::thread::spawn(move || {
                            let _ = db_clone.sync_tracks(&tracks_clone);
                        });
                    }
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

        if let Some(rx) = &self.config_watcher_rx {
            let mut reload_config = false;
            let mut reload_theme = false;
            let mut reload_presets = false;

            while let Ok(res) = rx.try_recv() {
                if let Ok(event) = res {
                    for path in event.paths {
                        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if fname == "config.toml" {
                            reload_config = true;
                        } else if fname == "eq_presets.toml" {
                            reload_presets = true;
                        } else if fname.ends_with(".toml") {
                            reload_theme = true;
                        }
                    }
                }
            }

            if reload_config {
                if let Ok((new_cfg, warnings)) = crate::config::Config::load_or_default() {
                    for w in &warnings {
                        tracing::warn!(target: "config", "hot-reload warning: {w}");
                    }
                    if new_cfg.theme != self.theme.name {
                        if let Ok(t) = crate::theme::Theme::load(&new_cfg.theme) {
                            self.theme = t;
                            self.set_info(format!("Config & Tema: 🎨 {}", new_cfg.theme));
                        }
                    } else {
                        self.set_info("Configuração recarregada (config.toml)");
                    }
                    let (bindings, _) = Bindings::from_config(&new_cfg.keybinds);
                    self.bindings = bindings;
                    self.player.crossfade_secs = new_cfg.playback.crossfade_secs;
                    self.config = new_cfg;
                }
            } else if reload_theme {
                let name = self.theme.name.clone();
                if let Ok(t) = crate::theme::Theme::load(&name) {
                    self.theme = t;
                    self.set_info(format!("Tema recarregado: 🎨 {name}"));
                }
                // Refresh cached theme names list
                if let Ok(dir) = crate::config::themes_dir() {
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
                    if !names.is_empty() {
                        self.theme_names = names;
                    }
                }
            } else if reload_presets {
                self.custom_eq_presets = crate::config::EqPresets::load().presets;
                self.set_info("Presets de equalização recarregados");
            }
        }

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

        if let Some(rx) = &self.subsonic_rx {
            match rx.try_recv() {
                Ok(Ok(res)) => {
                    match res {
                        crate::subsonic::SubsonicFetchResult::Songs(tracks) => {
                            let n = tracks.len();
                            self.subsonic_browser_results = tracks;
                            self.subsonic_browser_row = 0;
                            self.set_info(format!("Subsonic: {n} faixa(s) encontradas."));
                        }
                        crate::subsonic::SubsonicFetchResult::Albums(albums) => {
                            let n = albums.len();
                            self.subsonic_browser_albums = albums;
                            self.subsonic_browser_row = 0;
                            self.set_info(format!("Subsonic: {n} álbum(ns) carregados."));
                        }
                        crate::subsonic::SubsonicFetchResult::Playlists(playlists) => {
                            let n = playlists.len();
                            self.subsonic_browser_playlists = playlists;
                            self.subsonic_browser_row = 0;
                            self.set_info(format!("Subsonic: {n} playlist(s) carregadas."));
                        }
                    }
                    self.subsonic_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Subsonic: erro — {e}"));
                    self.subsonic_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.subsonic_rx = None;
                }
            }
        }

        if let Some(rx) = &self.vault_rx {
            match rx.try_recv() {
                Ok(Ok(tracks)) => {
                    let n = tracks.len();
                    self.vault_results = tracks;
                    self.vault_row = 0;
                    self.set_info(format!("Cloud Vault: {n} faixa(s) encontradas."));
                    self.vault_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Cloud Vault: erro — {e}"));
                    self.vault_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.vault_rx = None;
                }
            }
        }

        if let Some(rx) = &self.browse_rx {
            match rx.try_recv() {
                Ok(Ok(items)) => {
                    let n = items.len();
                    self.browse_results = items;
                    self.browse_row = 0;
                    self.set_info(format!("Descoberta: {n} playlist(s) públicas encontradas."));
                    self.browse_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Descoberta: erro — {e}"));
                    self.browse_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.browse_rx = None;
                }
            }
        }

        if let Some(rx) = &self.share_publish_rx {
            match rx.try_recv() {
                Ok(Ok(id)) => {
                    self.set_info(format!("Playlist publicada com sucesso! ID: {id}"));
                    self.show_share_modal = false;
                    self.share_publish_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Publicação: erro — {e}"));
                    self.share_publish_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.share_publish_rx = None;
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
                    let is_stream = self
                        .loading_track
                        .as_ref()
                        .map(Self::track_is_stream)
                        .unwrap_or(false);
                    if is_stream && self.stream_reconnect_attempts < 3 {
                        self.stream_reconnect_attempts += 1;
                        self.set_info(format!(
                            "⏳ Conexão com a rádio oscilou. Reconectando ({}/3)…",
                            self.stream_reconnect_attempts
                        ));
                        self.load_rx = None;
                        self.loading_track = None;
                        self.pending_seek_offset = None;
                        self.play_current();
                    } else {
                        self.stream_reconnect_attempts = 0;
                        self.set_error(format!("Playlist/Stream: falha ao carregar — {e}"));
                        self.load_rx = None;
                        self.loading_track = None;
                        self.pending_seek_offset = None;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.load_rx = None;
                    self.loading_track = None;
                    self.pending_seek_offset = None;
                }
            }
        }

        if let Some(rx) = &self.prefetch.rx {
            while let Ok((kind, path, res)) = rx.try_recv() {
                match kind {
                    SlotKind::Next => {
                        self.prefetch.building_next = None;
                        if let Ok(source) = res {
                            let cur = self.queue_index.unwrap_or(0);
                            let expected_next =
                                self.pick_next_index(cur).and_then(|i| self.queue.get(i));
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
                                if self.queue.len() > 1 {
                                    Some(self.queue.len() - 1)
                                } else {
                                    None
                                }
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

        if let Some(rx) = &self.update_check_rx {
            match rx.try_recv() {
                Ok(Ok(Some(info))) => {
                    self.set_info(format!(
                        "✨ Update v{} available! Press Shift+U to update.",
                        info.latest_version
                    ));
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

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let entry = (song.clone(), artist.clone(), now);
            if !self.radio_history.iter().any(|(s, a, _)| s == &song && a == &artist) {
                self.radio_history.insert(0, entry);
                if self.radio_history.len() > 30 {
                    self.radio_history.pop();
                }
            }

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

        if let Some(s) = &mut self.media_session {
            if self.player.current().is_some() {
                s.update_playback(!self.player.is_paused(), self.player.elapsed());
            }
        }

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

        if let Some(rx) = &self.lyrics_rx {
            match rx.try_recv() {
                Ok((track_path, Some(lyrics))) => {
                    if track_path.exists() && !track_path.to_string_lossy().starts_with("http") {
                        let lrc_path = track_path.with_extension("lrc");
                        if !lrc_path.exists() {
                            let _ = lyrics.save_to_file(&lrc_path);
                        }
                    }
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

        if let Some(rx) = &self.download_rx {
            match rx.try_recv() {
                Ok(Ok(path)) => {
                    let file_name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("audio.mp3")
                        .to_string();
                    let meta = crate::metadata::probe_full(&path);
                    let track = Track {
                        path: path.clone(),
                        title: meta.title.unwrap_or_else(|| file_name.clone()),
                        artist: meta.artist,
                        album: meta.album,
                        genre: meta.genre,
                        year: meta.year,
                        duration: meta.duration,
                        replaygain_track_db: None,
                        replaygain_album_db: None,
                        cover_url: None,
                        added_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs()),
                    };
                    self.library.push(track.clone());
                    if let Some(db) = &self.db {
                        let _ = db.sync_tracks(&[track]);
                    }
                    self.library_revision = self.library_revision.wrapping_add(1);
                    self.set_info(format!("💾 Download concluído e salvo: {file_name}"));
                    self.download_rx = None;
                }
                Ok(Err(e)) => {
                    self.set_error(format!("Falha no download: {e}"));
                    self.download_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.download_rx = None;
                }
            }
        }

        let mut ipc_requests = Vec::new();
        if let Some(server) = &self.ipc_server {
            while let Ok(req) = server.rx.try_recv() {
                ipc_requests.push(req);
            }
        }

        for req in ipc_requests {
            use crate::ipc::IpcCommand as C;
            let reply = match req.command {
                C::Play => {
                    if self.player.is_paused() {
                        self.player.toggle();
                    }
                    "OK: Playing".to_string()
                }
                C::Pause => {
                    if !self.player.is_paused() {
                        self.player.toggle();
                    }
                    "OK: Paused".to_string()
                }
                C::Toggle => {
                    self.player.toggle();
                    format!(
                        "OK: {}",
                        if self.player.is_paused() {
                            "Paused"
                        } else {
                            "Playing"
                        }
                    )
                }
                C::Next => {
                    self.next();
                    let title = self
                        .player
                        .current()
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "None".into());
                    format!("OK: Next track -> {title}")
                }
                C::Prev => {
                    self.prev();
                    let title = self
                        .player
                        .current()
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "None".into());
                    format!("OK: Prev track -> {title}")
                }
                C::Stop => {
                    self.player.stop();
                    "OK: Stopped".to_string()
                }
                C::Volume(arg) => {
                    let cur_vol = self.player.volume();
                    if arg.is_empty() {
                        format!("Volume: {:.0}%", cur_vol * 100.0)
                    } else if let Some(stripped) = arg.strip_prefix('+') {
                        if let Ok(delta) = stripped.parse::<f32>() {
                            self.player
                                .set_volume((cur_vol + delta / 100.0).clamp(0.0, 1.5));
                        }
                        format!("Volume: {:.0}%", self.player.volume() * 100.0)
                    } else if let Some(stripped) = arg.strip_prefix('-') {
                        if let Ok(delta) = stripped.parse::<f32>() {
                            self.player
                                .set_volume((cur_vol - delta / 100.0).clamp(0.0, 1.5));
                        }
                        format!("Volume: {:.0}%", self.player.volume() * 100.0)
                    } else if let Ok(val) = arg.parse::<f32>() {
                        let target = (val / 100.0).clamp(0.0, 1.5);
                        self.player.set_volume(target);
                        format!("Volume: {:.0}%", self.player.volume() * 100.0)
                    } else {
                        "ERROR: Invalid volume argument".to_string()
                    }
                }
                C::Status => {
                    let cur_vol = self.player.volume();
                    if let Some(t) = self.player.current() {
                        let state = if self.player.is_paused() {
                            "⏸ Paused"
                        } else {
                            "▶ Playing"
                        };
                        let el = self.player.elapsed();
                        let dur_str = t
                            .duration
                            .map(|d| format!("{:02}:{:02}", d.as_secs() / 60, d.as_secs() % 60))
                            .unwrap_or_else(|| "--:--".into());
                        let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
                        format!(
                            "{state}: {} - {} [{:02}:{:02}/{dur_str}] (Vol: {:.0}%)",
                            artist,
                            t.title,
                            el.as_secs() / 60,
                            el.as_secs() % 60,
                            cur_vol * 100.0
                        )
                    } else {
                        "⏹ Stopped: No track playing".to_string()
                    }
                }
                C::StatusJson => {
                    let cur_vol = self.player.volume();
                    if let Some(t) = self.player.current() {
                        let state = if self.player.is_paused() {
                            "paused"
                        } else {
                            "playing"
                        };
                        let el = self.player.elapsed().as_secs();
                        let dur = t.duration.map(|d| d.as_secs()).unwrap_or(0);
                        let artist = t.artist.as_deref().unwrap_or("");
                        let album = t.album.as_deref().unwrap_or("");
                        format!(
                            "{{\"status\":\"{state}\",\"title\":\"{}\",\"artist\":\"{}\",\"album\":\"{}\",\"elapsed_secs\":{el},\"duration_secs\":{dur},\"volume\":{:.0}}}",
                            t.title.replace('"', "\\\""),
                            artist.replace('"', "\\\""),
                            album.replace('"', "\\\""),
                            cur_vol * 100.0
                        )
                    } else {
                        "{\"status\":\"stopped\",\"title\":\"\",\"artist\":\"\",\"album\":\"\",\"elapsed_secs\":0,\"duration_secs\":0,\"volume\":0}".to_string()
                    }
                }
            };
            let _ = req.reply_tx.send(reply);
        }

        if let Some(when) = self.sleep_until {
            if std::time::Instant::now() >= when {
                self.player.stop();
                self.sleep_until = None;
                self.set_info("Sleep timer reached — playback stopped.");
                return Ok(());
            }
        }

        if self.player.is_crossfading() {
            if let CrossfadeStatus::Complete = self.player.update_crossfade() {
                if let Some(idx) = self.pending_crossfade_idx.take() {
                    self.queue_index = Some(idx);
                    self.queue_state.select(Some(idx));
                    if let Some(t) = self.player.current().cloned() {
                        self.lyrics = crate::lyrics::Lyrics::for_track(&t.path);
                        self.set_info(format!("Playing: {}", t.display()));
                        self.on_track_started(t);
                    }
                }
            }
        }

        if !matches!(self.repeat, RepeatMode::One) && self.player.crossfade_secs > 0.0 {
            if let Some(remaining) = self.player.remaining() {
                let xfade = Duration::from_secs_f32(self.player.crossfade_secs);
                if remaining > Duration::ZERO && remaining <= xfade {
                    let cur = self.queue_index.unwrap_or(0);
                    if let Some(next_idx) = self.pick_next_index(cur) {
                        if let Some(track) = self.queue.get(next_idx).cloned() {
                            if self.pending_crossfade_idx.is_none() {
                                let scale = rg_scale(&track, self.replaygain_mode);
                                if self.player.begin_crossfade(&track, scale).is_ok() {
                                    self.pending_crossfade_idx = Some(next_idx);
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.player.gapless_queued.is_some() && self.player.sink_queue_len() <= 1 {
            if let Some(next_track) = self.player.gapless_queued.clone() {
                self.player.advance_gapless(next_track.clone());
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

        if !self.current_play_recorded {
            if let Some(track) = self.player.current().cloned() {
                if self.player.elapsed().as_secs_f64() >= self.play_threshold_secs {
                    self.play_history.record_play(&track.path);
                    self.play_history_revision = self.play_history_revision.wrapping_add(1);
                    self.current_play_recorded = true;

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

        if self.player.is_empty() && self.player.current().is_some() {
            if let Some(cur) = self.player.current() {
                if Self::track_is_live_radio(cur)
                    && self.stream_reconnect_attempts < 3
                    && !self.player.is_paused()
                {
                    self.stream_reconnect_attempts += 1;
                    self.set_info(format!(
                        "⏳ Sinal da rádio interrompido. Reconectando ({}/3)…",
                        self.stream_reconnect_attempts
                    ));
                    self.play_current();
                    return Ok(());
                }
            }
            self.stream_reconnect_attempts = 0;
            self.advance();
        }
        Ok(())
    }
}
