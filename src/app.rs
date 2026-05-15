use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use ratatui::widgets::ListState;
use std::{path::PathBuf, time::Duration};
use walkdir::WalkDir;

use crate::{
    audio::Player,
    audio::Track,
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
}

impl SortMode {
    pub fn cycle(self) -> Self {
        match self {
            SortMode::Title => SortMode::Artist,
            SortMode::Artist => SortMode::Album,
            SortMode::Album => SortMode::Title,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Title => "title",
            SortMode::Artist => "artist",
            SortMode::Album => "album",
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
}

#[derive(Debug, Clone)]
struct UndoSnapshot {
    queue: Vec<Track>,
    queue_index: Option<usize>,
    label: String,
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
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            ViewMode::Flat => ViewMode::Albums,
            ViewMode::Albums => ViewMode::Flat,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Flat => "flat",
            ViewMode::Albums => "albums",
        }
    }
}

impl App {
    pub fn new(config: Config, theme: Theme) -> Result<Self> {
        let player = Player::new(config.playback.default_volume)?;
        let tap = player.tap();

        let cache_file = cache_path();
        let mut cache = cache_file
            .as_ref()
            .map(|p| MetadataCache::load(p))
            .unwrap_or_default();
        let library = scan_library(&config.music_dirs, &mut cache);
        if let Some(p) = &cache_file {
            cache.save(p);
        }

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

        let mut library_state = ListState::default();
        if !library.is_empty() {
            library_state.select(Some(0));
        }

        Ok(Self {
            config,
            theme,
            player,
            tap,
            library,
            queue: Vec::new(),
            undo_stack: std::collections::VecDeque::with_capacity(MAX_UNDO_SNAPSHOTS),
            library_state,
            queue_state: ListState::default(),
            focus: Pane::Library,
            queue_index: None,
            status: "Noctune ready. Press ? for help.".into(),
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
        })
    }

    pub fn search_active(&self) -> bool {
        self.search_editing
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    pub fn visible_library(&self) -> Vec<&Track> {
        let base: Vec<&Track> = if self.search.is_empty() {
            self.library.iter().collect()
        } else {
            let needle = self.search.to_lowercase();
            self.library
                .iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&needle)
                        || t.artist
                            .as_deref()
                            .map(|a| a.to_lowercase().contains(&needle))
                            .unwrap_or(false)
                })
                .collect()
        };
        base
    }

    pub fn library_rows(&self) -> Vec<LibraryRow> {
        let visible = self.visible_library();
        if self.view_mode == ViewMode::Flat || self.sort != SortMode::Album {
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
        if let Some(when) = self.sleep_until {
            if std::time::Instant::now() >= when {
                self.player.stop();
                self.sleep_until = None;
                self.status = "Sleep timer reached — playback stopped.".into();
            }
        }
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
                sort_tracks(&mut self.library, self.sort);
                self.library_state.select(if self.library.is_empty() {
                    None
                } else {
                    Some(0)
                });
                self.status = format!("Sort: {}", self.sort.label());
            }
            Action::SleepTimer => self.toggle_sleep_timer(),
            Action::SavePlaylist => self.save_playlist(),
            Action::LoadPlaylist => self.load_first_playlist(),
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
            Action::RemoveQueueItem => self.remove_from_queue(),
            Action::ClearQueue => self.clear_queue(),
            Action::UndoQueueAction => self.undo_queue_action(),
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
        }
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
            _ => {}
        }
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
                    self.undo_stack.clear();
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
                    let label = format!("removed {}", self.queue[i].display());
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
                }
            }
        }
    }

    fn clear_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.push_undo_snapshot(format!("cleared queue ({} tracks)", self.queue.len()));
        self.queue.clear();
        self.queue_state.select(None);
        self.queue_index = None;
        self.player.stop();
    }

    fn push_undo_snapshot(&mut self, label: String) {
        self.undo_stack.push_back(UndoSnapshot {
            queue: self.queue.clone(),
            queue_index: self.queue_index,
            label,
        });
        while self.undo_stack.len() > MAX_UNDO_SNAPSHOTS {
            self.undo_stack.pop_front();
        }
    }

    fn undo_queue_action(&mut self) {
        let Some(snapshot) = self.undo_stack.pop_back() else {
            self.status = "Nothing to undo.".into();
            return;
        };
        self.queue = snapshot.queue;
        self.queue_index = snapshot.queue_index.filter(|i| *i < self.queue.len());
        self.queue_state.select(if self.queue.is_empty() {
            None
        } else {
            Some(self.queue_index.unwrap_or(0).min(self.queue.len() - 1))
        });
        self.status = format!("Undo: {}", snapshot.label);
    }

    fn play_current(&mut self) {
        if let Some(i) = self.queue_index {
            if let Some(t) = self.queue.get(i).cloned() {
                match self.player.play(&t) {
                    Ok(_) => {
                        self.status = format!("Playing: {}", t.display());
                        self.lyrics = crate::lyrics::Lyrics::for_track(&t.path);
                        self.push_history(t);
                    }
                    Err(e) => self.status = format!("Error: {e}"),
                }
            }
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

    fn save_playlist(&mut self) {
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
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("queue-{stamp}.m3u"));
        let mut text = String::from("#EXTM3U\n");
        for t in &self.queue {
            text.push_str(&t.path.display().to_string());
            text.push('\n');
        }
        match std::fs::write(&path, text) {
            Ok(_) => self.status = format!("Saved playlist: {}", path.display()),
            Err(e) => self.status = format!("Save error: {e}"),
        }
    }

    fn load_first_playlist(&mut self) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Playlist dir error: {e}");
                return;
            }
        };
        let entries = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                self.status = format!("No playlists at {}", dir.display());
                return;
            }
        };
        let mut latest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("m3u") {
                continue;
            }
            let mtime = e.metadata().and_then(|m| m.modified()).ok();
            if let Some(mtime) = mtime {
                if latest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                    latest = Some((mtime, path));
                }
            }
        }
        let Some((_, path)) = latest else {
            self.status = format!("No .m3u files in {}", dir.display());
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.status = format!("Could not read {}", path.display());
            return;
        };
        let mut loaded = 0usize;
        let original_queue_len = self.queue.len();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("http://") || line.starts_with("https://") {
                self.queue.push(Track::from_url(line.to_string()));
                loaded += 1;
                continue;
            }
            let p = std::path::PathBuf::from(line);
            if p.exists() {
                self.queue.push(Track::from_path_with_meta(p));
                loaded += 1;
            }
        }
        if loaded > 0 {
            let loaded_tracks = self.queue.split_off(original_queue_len);
            self.push_undo_snapshot(format!("loaded playlist ({} tracks)", loaded));
            self.queue.extend(loaded_tracks);
        }
        if loaded > 0 && self.queue_state.selected().is_none() {
            self.queue_state.select(Some(0));
        }
        self.status = format!("Loaded {} tracks from {}", loaded, path.display());
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
    }
}
