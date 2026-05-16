use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use ratatui::widgets::ListState;
use std::{path::PathBuf, time::Duration};
use walkdir::WalkDir;

use notify::Watcher as _;

use crate::{
    audio::{CrossfadeStatus, Player, Track},
    cache::{cache_path, MetadataCache},
    config::Config,
    keybinds::{Action, Bindings},
    theme::Theme,
    tui::Tui,
    ui,
    visualizer::VizTap,
};

const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac"];

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
}

impl VizMode {
    pub fn cycle(self) -> Self {
        match self {
            VizMode::Spectrum => VizMode::Waveform,
            VizMode::Waveform => VizMode::VuMeter,
            VizMode::VuMeter => VizMode::Spectrum,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            VizMode::Spectrum => "spectrum",
            VizMode::Waveform => "waveform",
            VizMode::VuMeter => "vu-meter",
        }
    }
}

pub struct App {
    #[allow(dead_code)]
    pub config: Config,
    pub theme: Theme,
    pub player: Player,
    pub tap: VizTap,
    pub library: Vec<Track>,
    pub queue: Vec<Track>,
    pub library_state: ListState,
    pub queue_state: ListState,
    pub focus: Pane,
    pub queue_index: Option<usize>,
    pub status: String,
    pub should_quit: bool,
    pub search: String,
    pub search_editing: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub show_help: bool,
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
    pub scan_rx: Option<std::sync::mpsc::Receiver<Vec<Track>>>,
    pub fs_event_rx: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pub _fs_watcher: Option<notify::RecommendedWatcher>,
    pub rescan_debounce_until: Option<std::time::Instant>,
    pub tick_count: u64,
    pub queue_undo: Option<(Vec<Track>, Option<usize>)>,
    pub hover_x: Option<u16>,
    pub show_audio_panel: bool,
    pub audio_panel_row: usize,
    pub replaygain_mode: ReplayGainMode,
    pub viz_mode: VizMode,
    pub ratings: crate::ratings::Ratings,
    pub playlist_name_editing: bool,
    pub playlist_name_input: String,
    pub show_playlist_browser: bool,
    pub playlist_browser_entries: Vec<PlaylistEntry>,
    pub playlist_browser_row: usize,
    pub playlist_browser_delete_confirm: Option<usize>,
    pub active_playlist_name: Option<String>,
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
}

#[derive(Debug, Clone)]
pub enum LibraryRow {
    Header(String),
    Track(Track),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Flat,
    Albums,
    RecentlyPlayed,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            ViewMode::Flat => ViewMode::Albums,
            ViewMode::Albums => ViewMode::Flat,
            ViewMode::RecentlyPlayed => ViewMode::Flat,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Flat => "flat",
            ViewMode::Albums => "albums",
            ViewMode::RecentlyPlayed => "recently played",
        }
    }
}

impl App {
    pub fn new(config: Config, theme: Theme) -> Result<Self> {
        let player = Player::new(config.playback.default_volume, config.visualizer.sensitivity)?;
        let tap = player.tap();

        let config_shuffle = config.playback.shuffle;
        let config_repeat = config.playback.repeat;
        let config_keybinds = config.keybinds.clone();
        let spotify_client_id = config.spotify.client_id.clone();
        let spotify_redirect_uri = config.spotify.redirect_uri();
        let spotify_port = config.spotify.redirect_port;
        let _ = spotify_port;

        let spotify = crate::spotify::load_tokens()
            .filter(|_| !spotify_client_id.is_empty())
            .and_then(|t| crate::spotify::SpotifyApi::new(spotify_client_id.clone(), t).ok());

        // Start async library scan
        let (scan_tx, scan_rx) = std::sync::mpsc::channel::<Vec<Track>>();
        let scan_dirs = config.music_dirs.clone();
        std::thread::spawn(move || {
            let cache_file = cache_path();
            let mut cache = cache_file
                .as_ref()
                .map(|p| MetadataCache::load(p))
                .unwrap_or_default();
            let tracks = scan_library(&scan_dirs, &mut cache);
            if let Some(p) = &cache_file {
                cache.save(p);
            }
            let _ = scan_tx.send(tracks);
        });

        // Start filesystem watcher
        let (fs_tx, fs_event_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut _fs_watcher = notify::RecommendedWatcher::new(fs_tx, notify::Config::default()).ok();
        if let Some(w) = &mut _fs_watcher {
            for dir in &config.music_dirs {
                let _ = w.watch(dir.as_path(), notify::RecursiveMode::Recursive);
            }
        }

        Ok(Self {
            config,
            theme,
            player,
            tap,
            library: Vec::new(),
            queue: Vec::new(),
            library_state: ListState::default(),
            queue_state: ListState::default(),
            focus: Pane::Library,
            queue_index: None,
            status: "Scanning library…".into(),
            should_quit: false,
            search: String::new(),
            search_editing: false,
            shuffle: config_shuffle,
            repeat: if config_repeat { RepeatMode::All } else { RepeatMode::Off },
            show_help: false,
            sort: SortMode::Title,
            sleep_until: None,
            history: std::collections::VecDeque::with_capacity(64),
            bindings: Bindings::from_config(&config_keybinds),
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
            scan_rx: Some(scan_rx),
            fs_event_rx: Some(fs_event_rx),
            _fs_watcher,
            rescan_debounce_until: None,
            tick_count: 0,
            queue_undo: None,
            hover_x: None,
            show_audio_panel: false,
            audio_panel_row: 0,
            replaygain_mode: ReplayGainMode::Track,
            viz_mode: VizMode::Spectrum,
            ratings: crate::ratings::Ratings::load(),
            playlist_name_editing: false,
            playlist_name_input: String::new(),
            show_playlist_browser: false,
            playlist_browser_entries: Vec::new(),
            playlist_browser_row: 0,
            playlist_browser_delete_confirm: None,
            active_playlist_name: None,
        })
    }

    pub fn is_loading(&self) -> bool {
        self.url_rx.is_some() || self.scan_rx.is_some()
    }

    pub fn search_active(&self) -> bool {
        self.search_editing
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    pub fn visible_library(&self) -> Vec<&Track> {
        let source: Box<dyn Iterator<Item = &Track>> = if self.view_mode == ViewMode::RecentlyPlayed {
            Box::new(self.history.iter())
        } else {
            Box::new(self.library.iter())
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

    pub fn library_rows(&self) -> Vec<LibraryRow> {
        let visible = self.visible_library();
        if self.view_mode == ViewMode::Flat
            || self.view_mode == ViewMode::RecentlyPlayed
            || self.sort != SortMode::Album
        {
            return visible
                .into_iter()
                .map(|t| LibraryRow::Track(t.clone()))
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
            out.push(LibraryRow::Track(t.clone()));
        }
        out
    }

    fn selected_library_track(&self) -> Option<Track> {
        let rows = self.library_rows();
        let idx = self.library_state.selected()?;
        match rows.get(idx)? {
            LibraryRow::Track(t) => Some(t.clone()),
            LibraryRow::Header(_) => None,
        }
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| ui::render(f, self))?;
            self.tick()?;
            if event::poll(Duration::from_millis(33))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
                    Event::Mouse(m) => self.on_mouse(m),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        self.tick_count = self.tick_count.wrapping_add(1);

        // Poll completed library scan
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(mut tracks) => {
                    sort_tracks(&mut tracks, self.sort);
                    let n = tracks.len();
                    self.library = tracks;
                    self.library_state.select(if self.library.is_empty() { None } else { Some(0) });
                    self.status = format!("Library: {n} tracks.");
                    self.scan_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.status = "Library scan failed.".into();
                    self.scan_rx = None;
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
                            self.rescan_debounce_until = Some(
                                std::time::Instant::now() + Duration::from_secs(2),
                            );
                        }
                    }
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }

        // Trigger debounced rescan
        if let Some(until) = self.rescan_debounce_until {
            if std::time::Instant::now() >= until && self.scan_rx.is_none() {
                self.rescan_debounce_until = None;
                self.start_async_scan();
                self.status = "Library changed — rescanning…".into();
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
                    self.status = format!("Added {n} track(s) to queue.");
                    self.url_rx = None;
                }
                Ok(Err(e)) => {
                    self.status = format!("Load error: {e}");
                    self.url_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.status = "URL load failed (worker disconnected).".into();
                    self.url_rx = None;
                }
            }
        }
        if let Some(when) = self.sleep_until {
            if std::time::Instant::now() >= when {
                self.player.stop();
                self.sleep_until = None;
                self.status = "Sleep timer reached — playback stopped.".into();
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
                        self.status = format!("Playing: {}", t.display());
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

        // Regular end detection (covers repeat:one and tracks with no duration)
        if self.player.is_empty() && self.player.current().is_some() {
            self.advance();
        }
        Ok(())
    }

    pub fn sleep_remaining(&self) -> Option<Duration> {
        self.sleep_until.map(|t| t.saturating_duration_since(std::time::Instant::now()))
    }

    fn toggle_sleep_timer(&mut self) {
        if self.sleep_until.is_some() {
            self.sleep_until = None;
            self.status = "Sleep timer cancelled.".into();
        } else {
            let when = std::time::Instant::now() + Duration::from_secs(30 * 60);
            self.sleep_until = Some(when);
            self.status = "Sleep timer: 30 min.".into();
        }
    }

    fn push_history(&mut self, t: Track) {
        if self.history.front().map(|h| h.path == t.path).unwrap_or(false) {
            return;
        }
        self.history.push_front(t);
        while self.history.len() > 64 {
            self.history.pop_back();
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return;
        }

        if self.show_help {
            self.show_help = false;
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

        if self.show_playlist_browser {
            self.handle_playlist_browser_key(key);
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
                KeyCode::Backspace => { self.playlist_name_input.pop(); }
                KeyCode::Char(c) => { self.playlist_name_input.push(c); }
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
                KeyCode::Backspace => { self.url_input.pop(); }
                KeyCode::Char(c) => { self.url_input.push(c); }
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
            Action::Stop => self.player.stop(),
            Action::Shuffle => {
                self.shuffle = !self.shuffle;
                self.status = format!("Shuffle: {}", if self.shuffle { "on" } else { "off" });
            }
            Action::Repeat => {
                self.repeat = self.repeat.cycle();
                self.status = format!("Repeat: {}", self.repeat.label());
            }
            Action::Sort => {
                self.sort = self.sort.cycle();
                sort_tracks_with_ratings(&mut self.library, self.sort, Some(&self.ratings));
                self.library_state.select(if self.library.is_empty() {
                    None
                } else {
                    Some(0)
                });
                self.status = format!("Sort: {}", self.sort.label());
            }
            Action::SleepTimer => self.toggle_sleep_timer(),
            Action::SavePlaylist => {
                self.playlist_name_editing = true;
                self.playlist_name_input.clear();
                self.status = "Playlist name (Enter to save, Esc to cancel):".into();
            }
            Action::LoadPlaylist => {
                self.open_playlist_browser();
            }
            Action::VolumeUp => {
                let v = (self.player.volume() + 0.05).min(1.5);
                self.player.set_volume(v);
            }
            Action::VolumeDown => {
                let v = (self.player.volume() - 0.05).max(0.0);
                self.player.set_volume(v);
            }
            Action::SeekBack => {
                if let Err(e) = self.player.seek_relative(-5) {
                    self.status = format!("Seek error: {e}");
                }
            }
            Action::SeekForward => {
                if let Err(e) = self.player.seek_relative(5) {
                    self.status = format!("Seek error: {e}");
                }
            }
            Action::SelectionUp => self.move_selection(-1),
            Action::SelectionDown => self.move_selection(1),
            Action::ActivateSelection => self.activate_selection(),
            Action::Enqueue => self.enqueue_selection(),
            Action::RemoveQueueItem => {
                self.save_queue_undo();
                self.remove_from_queue();
            }
            Action::ClearQueue => self.clear_queue(),
            Action::SpotifyLogin => self.spotify_login(),
            Action::SpotifyToggle => self.spotify_toggle(),
            Action::ToggleView => {
                self.view_mode = self.view_mode.toggle();
                self.library_state.select(if self.visible_library().is_empty() {
                    None
                } else {
                    Some(0)
                });
                self.status = format!("View: {}", self.view_mode.label());
            }
            Action::EqLowUp => {
                self.player.eq().adjust_low(1.0);
                self.status = "EQ low +1 dB".into();
            }
            Action::EqLowDown => {
                self.player.eq().adjust_low(-1.0);
                self.status = "EQ low -1 dB".into();
            }
            Action::EqMidUp => {
                self.player.eq().adjust_mid(1.0);
                self.status = "EQ mid +1 dB".into();
            }
            Action::EqMidDown => {
                self.player.eq().adjust_mid(-1.0);
                self.status = "EQ mid -1 dB".into();
            }
            Action::EqHighUp => {
                self.player.eq().adjust_high(1.0);
                self.status = "EQ high +1 dB".into();
            }
            Action::EqHighDown => {
                self.player.eq().adjust_high(-1.0);
                self.status = "EQ high -1 dB".into();
            }
            Action::OpenUrl => {
                self.url_editing = true;
                self.status = "Paste URL (YouTube, YT Music, Spotify) — Enter to load, Esc to cancel".into();
            }
            Action::EqPreset => {
                let presets = crate::eq::PRESETS;
                self.eq_preset_idx = (self.eq_preset_idx + 1) % presets.len();
                let (name, state) = presets[self.eq_preset_idx];
                self.player.eq().set(state);
                self.status = format!("EQ preset: {name}");
            }
            Action::Rescan => self.start_async_scan(),
            Action::TrackInfo => self.show_info = true,
            Action::CycleTheme => self.cycle_theme(),
            Action::VizSensUp => self.adjust_viz_sensitivity(crate::visualizer::SENS_STEP),
            Action::VizSensDown => self.adjust_viz_sensitivity(-crate::visualizer::SENS_STEP),
            Action::UndoQueue => self.undo_queue(),
            Action::RecentlyPlayed => {
                if self.view_mode == ViewMode::RecentlyPlayed {
                    self.view_mode = ViewMode::Flat;
                    self.status = "View: library".into();
                } else {
                    self.view_mode = ViewMode::RecentlyPlayed;
                    self.status = format!("Recently played ({} tracks)", self.history.len());
                }
                self.library_state.select(if self.visible_library().is_empty() {
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
                self.status = format!("ReplayGain: {}", self.replaygain_mode.label());
            }
            Action::CycleVizMode => {
                self.viz_mode = self.viz_mode.cycle();
                self.status = format!("Visualizer: {}", self.viz_mode.label());
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
                    self.status = if fav {
                        "Added to favorites ♥".into()
                    } else {
                        "Removed from favorites".into()
                    };
                }
            }
        }
    }

    fn adjust_viz_sensitivity(&mut self, delta: f32) {
        let new_val = self.tap.adjust_sensitivity(delta);
        self.config.visualizer.sensitivity = new_val;
        self.status = format!("Visualizer sensitivity: ×{:.1}", new_val);
        if let Err(e) = self.config.save() {
            self.status = format!("sensitivity saved in memory only ({e})");
        }
    }

    pub const AUDIO_PANEL_ROWS: usize = 7;

    fn handle_audio_panel_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('e') => {
                self.show_audio_panel = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.audio_panel_row =
                    self.audio_panel_row.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.audio_panel_row + 1 < Self::AUDIO_PANEL_ROWS {
                    self.audio_panel_row += 1;
                }
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
                self.status = format!("EQ Low: {:+.0} dB", db);
            }
            1 => {
                self.player.eq().adjust_mid(dir as f32);
                let db = self.player.eq().snapshot().mid_db;
                self.status = format!("EQ Mid: {:+.0} dB", db);
            }
            2 => {
                self.player.eq().adjust_high(dir as f32);
                let db = self.player.eq().snapshot().high_db;
                self.status = format!("EQ High: {:+.0} dB", db);
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
                self.status = format!("EQ preset: {name}");
            }
            4 => {
                let v = (self.player.volume() + dir as f32 * 0.05).clamp(0.0, 1.5);
                self.player.set_volume(v);
                self.status = format!("Volume: {}%", (v * 100.0) as u32);
            }
            5 => {
                let xf = (self.player.crossfade_secs + dir as f32 * 0.5).clamp(0.0, 10.0);
                self.player.crossfade_secs = xf;
                self.status = format!("Crossfade: {:.1}s", xf);
            }
            6 => {
                self.adjust_viz_sensitivity(dir as f32 * crate::visualizer::SENS_STEP);
            }
            _ => {}
        }
    }

    fn cycle_theme(&mut self) {
        if self.theme_names.is_empty() {
            let dir = match crate::config::themes_dir() {
                Ok(d) => d,
                Err(e) => { self.status = format!("themes dir: {e}"); return; }
            };
            let mut names: Vec<String> = std::fs::read_dir(&dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("toml") {
                        p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
                    } else { None }
                })
                .collect();
            names.sort();
            if names.is_empty() {
                names.push("default".to_string());
            }
            self.theme_idx = names.iter().position(|n| n == &self.theme.name).unwrap_or(0);
            self.theme_names = names;
        }
        if self.theme_names.is_empty() { return; }
        self.theme_idx = (self.theme_idx + 1) % self.theme_names.len();
        let name = &self.theme_names[self.theme_idx];
        match crate::theme::Theme::load(name) {
            Ok(t) => {
                self.theme = t;
                self.status = format!("Theme: {name}");
            }
            Err(e) => self.status = format!("Theme load error: {e}"),
        }
    }

    fn start_async_scan(&mut self) {
        if self.scan_rx.is_some() {
            self.status = "Scan already in progress…".into();
            return;
        }
        let dirs = self.config.music_dirs.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<Track>>();
        self.scan_rx = Some(rx);
        self.status = "Scanning library…".into();
        std::thread::spawn(move || {
            let cache_file = cache_path();
            let mut cache = cache_file
                .as_ref()
                .map(|p| MetadataCache::load(p))
                .unwrap_or_default();
            let tracks = scan_library(&dirs, &mut cache);
            if let Some(p) = &cache_file {
                cache.save(p);
            }
            let _ = tx.send(tracks);
        });
    }

    fn start_url_load(&mut self, url: String) {
        if self.url_rx.is_some() {
            self.status = "Already loading, please wait…".into();
            return;
        }

        // HTTP stream (instant — no need for a thread)
        if !url.contains("spotify.com")
            && !url.starts_with("spotify:")
            && !crate::ytdlp::is_youtube_url(&url)
            && (url.starts_with("http://") || url.starts_with("https://"))
        {
            self.queue.push(crate::audio::Track::from_url(url));
            if self.queue_state.selected().is_none() {
                self.queue_state.select(Some(self.queue.len().saturating_sub(1)));
            }
            self.status = "Added stream URL to queue.".into();
            return;
        }

        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && !url.starts_with("spotify:")
            && !crate::ytdlp::is_youtube_url(&url)
        {
            self.status = format!("Unrecognised URL scheme: {url}");
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);

        // Spotify
        if url.contains("spotify.com") || url.starts_with("spotify:") {
            let Some(api) = self.spotify.clone() else {
                self.status = "Not logged in to Spotify. Press Shift+P first.".into();
                self.url_rx = None;
                return;
            };
            let (kind, id) = parse_spotify_url(&url);
            self.status = format!("Loading Spotify {kind}…");
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
        self.status = format!("Loading {url}…");
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
            self.status = "Set [spotify].client_id in config.toml first.".into();
            return;
        }
        match crate::spotify::authorize(&self.spotify_client_id, &self.spotify_redirect_uri) {
            Ok((url, session)) => {
                self.status = "Opening browser for Spotify login...".into();
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
                                    self.status = "Spotify login complete.".into();
                                }
                                Err(e) => self.status = format!("Spotify init error: {e}"),
                            }
                        }
                        Err(e) => self.status = format!("Token exchange failed: {e}"),
                    },
                    Err(e) => self.status = format!("Redirect listener error: {e}"),
                }
            }
            Err(e) => self.status = format!("Spotify authorize error: {e}"),
        }
    }

    fn spotify_toggle(&mut self) {
        let Some(api) = self.spotify.as_mut() else {
            self.status = "Not logged in to Spotify. Press Shift+P first.".into();
            return;
        };
        match api.currently_playing() {
            Ok(Some(cp)) if cp.is_playing => match api.pause() {
                Ok(_) => self.status = "Spotify paused.".into(),
                Err(e) => self.status = format!("Spotify pause error: {e}"),
            },
            Ok(_) => match api.play() {
                Ok(_) => self.status = "Spotify resumed.".into(),
                Err(e) => self.status = format!("Spotify play error: {e}"),
            },
            Err(e) => self.status = format!("Spotify error: {e}"),
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
        if let Err(e) = self.player.seek_absolute_fraction(frac) {
            self.status = format!("Seek error: {e}");
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
            if let Err(e) = self.player.seek_absolute_fraction(frac) {
                self.status = format!("Seek error: {e}");
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
                    if matches!(rows[cur as usize], LibraryRow::Track(_)) {
                        self.library_state.select(Some(cur as usize));
                        return;
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
                self.status = format!("Queued: {}", t.display());
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
                    self.queue.remove(i);
                    if self.queue.is_empty() {
                        self.queue_state.select(None);
                    } else {
                        self.queue_state.select(Some(i.min(self.queue.len() - 1)));
                    }
                }
            }
        }
    }

    fn clear_queue(&mut self) {
        let now = std::time::Instant::now();
        let confirmed = self
            .clear_confirm_until
            .map(|until| now < until)
            .unwrap_or(false);
        if !confirmed {
            self.clear_confirm_until = Some(now + Duration::from_secs(3));
            self.status = format!("Press c again within 3s to clear {} tracks", self.queue.len());
            return;
        }
        self.clear_confirm_until = None;
        let n = self.queue.len();
        self.save_queue_undo();
        self.queue.clear();
        self.queue_state.select(None);
        self.queue_index = None;
        self.player.stop();
        self.status = format!("Queue cleared ({n} tracks). Press u to undo.");
    }

    fn save_queue_undo(&mut self) {
        self.queue_undo = Some((self.queue.clone(), self.queue_index));
    }

    fn undo_queue(&mut self) {
        let Some((queue, idx)) = self.queue_undo.take() else {
            self.status = "Nothing to undo.".into();
            return;
        };
        let n = queue.len();
        self.queue = queue;
        self.queue_index = idx;
        self.queue_state.select(idx);
        self.status = format!("Undo: restored {n} tracks.");
    }

    fn play_current(&mut self) {
        let Some(i) = self.queue_index else { return };
        let Some(t) = self.queue.get(i).cloned() else { return };

        let path_str = t.path.to_string_lossy();
        if path_str.starts_with("spotify:track:") {
            // Route to Spotify Connect
            let uri = path_str.to_string();
            if let Some(api) = self.spotify.as_mut() {
                match api.play_uri(&uri) {
                    Ok(_) => {
                        self.status = format!("Spotify ▶ {}", t.display());
                        self.push_history(t);
                    }
                    Err(e) => self.status = format!("Spotify play error: {e}"),
                }
            } else {
                self.status = "Not logged in to Spotify. Press Shift+P first.".into();
            }
            return;
        }

        // Apply ReplayGain scaling
        self.player.rg_scale = rg_scale(&t, self.replaygain_mode);

        // Local file or HTTP/YouTube stream
        match self.player.play(&t) {
            Ok(_) => {
                self.status = format!("Playing: {}", t.display());
                self.lyrics = crate::lyrics::Lyrics::for_track(&t.path);
                self.push_history(t);
            }
            Err(e) => self.status = format!("Error: {e}"),
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
        self.play_current();
    }

    fn prev(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let i = self.queue_index.unwrap_or(0);
        let new = if i == 0 { self.queue.len() - 1 } else { i - 1 };
        self.queue_index = Some(new);
        self.queue_state.select(Some(new));
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
                self.play_current();
            } else {
                self.player.stop();
                self.queue_index = None;
            }
        }
    }

    fn save_playlist_named(&mut self, name: String) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Playlist dir error: {e}");
                return;
            }
        };
        if std::fs::create_dir_all(&dir).is_err() {
            self.status = format!("Could not create {}", dir.display());
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
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
                .collect()
        };
        let path = dir.join(format!("{safe_name}.m3u"));
        let mut text = String::from("#EXTM3U\n");
        for t in &self.queue {
            text.push_str(&t.path.display().to_string());
            text.push('\n');
        }
        match std::fs::write(&path, text) {
            Ok(_) => {
                self.active_playlist_name = Some(safe_name.clone());
                self.status = format!("Saved: {safe_name}.m3u");
            }
            Err(e) => self.status = format!("Save error: {e}"),
        }
    }

    fn open_playlist_browser(&mut self) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Playlist dir error: {e}");
                return;
            }
        };
        let mut entries: Vec<PlaylistEntry> = std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("m3u")
            })
            .filter_map(|e| {
                let path = e.path();
                let name = path.file_stem()?.to_str()?.to_string();
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let count = text.lines()
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .count();
                Some(PlaylistEntry { name, path, track_count: count })
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        if entries.is_empty() {
            self.status = "No playlists saved yet.".into();
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
                            self.status = format!("Deleted: {}", entry.name);
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
                    self.status = "Press Shift+D again to confirm deletion.".into();
                }
            }
            _ => {}
        }
    }

    fn load_playlist_at_row(&mut self, append: bool) {
        let Some(entry) = self.playlist_browser_entries.get(self.playlist_browser_row).cloned() else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&entry.path) else {
            self.status = format!("Could not read {}", entry.path.display());
            return;
        };
        if !append {
            self.queue.clear();
            self.queue_state.select(None);
            self.queue_index = None;
        }
        let start = self.queue.len();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if line.starts_with("http://") || line.starts_with("https://") {
                self.queue.push(Track::from_url(line.to_string()));
            } else {
                let p = std::path::PathBuf::from(line);
                if p.exists() {
                    self.queue.push(Track::from_path_with_meta(p));
                }
            }
        }
        let loaded = self.queue.len() - start;
        if !append {
            if !self.queue.is_empty() {
                self.queue_state.select(Some(0));
            }
            self.active_playlist_name = Some(entry.name.clone());
        }
        self.show_playlist_browser = false;
        self.status = if append {
            format!("Appended {} tracks from '{}'", loaded, entry.name)
        } else {
            format!("Loaded {} tracks from '{}'", loaded, entry.name)
        };
    }

}

fn rg_scale(track: &Track, mode: ReplayGainMode) -> f32 {
    let db = match mode {
        ReplayGainMode::Off => return 1.0,
        ReplayGainMode::Track => track.replaygain_track_db,
        ReplayGainMode::Album => track
            .replaygain_album_db
            .or(track.replaygain_track_db),
    };
    db.map(|db| 10f32.powf(db / 20.0)).unwrap_or(1.0)
}

fn parse_spotify_url(url: &str) -> (String, String) {
    if let Some(path) = url.strip_prefix("spotify:") {
        let mut parts = path.splitn(2, ':');
        let k = parts.next().unwrap_or("").to_string();
        let i = parts.next().unwrap_or("").to_string();
        (k, i)
    } else {
        let trimmed = url.split('?').next().unwrap_or(url);
        let segs: Vec<&str> = trimmed.rsplit('/').take(2).collect();
        let i = segs.first().copied().unwrap_or("").to_string();
        let k = segs.get(1).copied().unwrap_or("").to_string();
        (k, i)
    }
}

fn rect_contains(r: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    r.width > 0 && r.height > 0 && x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn pseudo_random(modulo: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(1);
    let mut x = nanos.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
    x ^= x >> 33;
    (x as usize) % modulo.max(1)
}

fn scan_library(dirs: &[PathBuf], cache: &mut MetadataCache) -> Vec<Track> {
    let mut out = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());
            if let Some(e) = ext {
                if AUDIO_EXTS.contains(&e.as_str()) {
                    out.push(cache.track_for(path));
                }
            }
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

fn sort_tracks(tracks: &mut [Track], mode: SortMode) {
    sort_tracks_with_ratings(tracks, mode, None);
}

fn sort_tracks_with_ratings(tracks: &mut [Track], mode: SortMode, ratings: Option<&crate::ratings::Ratings>) {
    match mode {
        SortMode::Title => tracks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        SortMode::Artist => tracks.sort_by(|a, b| {
            let aa = a.artist.as_deref().unwrap_or("~").to_lowercase();
            let bb = b.artist.as_deref().unwrap_or("~").to_lowercase();
            aa.cmp(&bb).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        SortMode::Album => tracks.sort_by(|a, b| {
            let aa = a.album.as_deref().unwrap_or("~").to_lowercase();
            let bb = b.album.as_deref().unwrap_or("~").to_lowercase();
            aa.cmp(&bb).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        SortMode::Rating => {
            tracks.sort_by(|a, b| {
                let ra = ratings.map(|r| r.get(&a.path)).unwrap_or(0);
                let rb = ratings.map(|r| r.get(&b.path)).unwrap_or(0);
                rb.cmp(&ra).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            });
        }
    }
}
