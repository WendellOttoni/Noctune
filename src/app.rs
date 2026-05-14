use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use std::{path::PathBuf, time::Duration};
use walkdir::WalkDir;

use crate::{
    audio::Player, audio::Track, config::Config, theme::Theme, tui::Tui, ui, visualizer::VizTap,
};

const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Library,
    Queue,
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
}

impl App {
    pub fn new(config: Config, theme: Theme) -> Result<Self> {
        let player = Player::new(config.playback.default_volume)?;
        let tap = player.tap();
        let library = scan_library(&config.music_dirs);
        let config_shuffle = config.playback.shuffle;
        let config_repeat = config.playback.repeat;

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
        })
    }

    pub fn search_active(&self) -> bool {
        self.search_editing
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    pub fn visible_library(&self) -> Vec<&Track> {
        if self.search.is_empty() {
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
        }
    }

    fn selected_library_track(&self) -> Option<Track> {
        let visible = self.visible_library();
        let idx = self.library_state.selected()?;
        visible.get(idx).map(|t| (*t).clone())
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| ui::render(f, self))?;
            self.tick()?;
            if event::poll(Duration::from_millis(33))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.on_key(key);
                    }
                }
            }
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        if self.player.is_empty() && self.player.current().is_some() {
            self.advance();
        }
        Ok(())
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

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => {
                self.search_editing = true;
                self.focus = Pane::Library;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Pane::Library => Pane::Queue,
                    Pane::Queue => Pane::Library,
                };
            }
            KeyCode::Char(' ') => self.player.toggle(),
            KeyCode::Char('n') => self.next(),
            KeyCode::Char('p') => self.prev(),
            KeyCode::Char('s') => self.player.stop(),
            KeyCode::Char('S') => {
                self.shuffle = !self.shuffle;
                self.status = format!("Shuffle: {}", if self.shuffle { "on" } else { "off" });
            }
            KeyCode::Char('r') => {
                self.repeat = self.repeat.cycle();
                self.status = format!("Repeat: {}", self.repeat.label());
            }
            KeyCode::Char('w') => self.save_playlist(),
            KeyCode::Char('L') => self.load_first_playlist(),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let v = (self.player.volume() + 0.05).min(1.5);
                self.player.set_volume(v);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let v = (self.player.volume() - 0.05).max(0.0);
                self.player.set_volume(v);
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Left => {
                if let Err(e) = self.player.seek_relative(-5) {
                    self.status = format!("Seek error: {e}");
                }
            }
            KeyCode::Right => {
                if let Err(e) = self.player.seek_relative(5) {
                    self.status = format!("Seek error: {e}");
                }
            }
            KeyCode::Enter => self.activate_selection(),
            KeyCode::Char('a') => self.enqueue_selection(),
            KeyCode::Char('d') => self.remove_from_queue(),
            KeyCode::Char('c') => self.clear_queue(),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = match self.focus {
            Pane::Library => self.visible_library().len(),
            Pane::Queue => self.queue.len(),
        };
        let state = match self.focus {
            Pane::Library => &mut self.library_state,
            Pane::Queue => &mut self.queue_state,
        };
        if len == 0 {
            return;
        }
        let cur = state.selected().unwrap_or(0) as i32;
        let new = (cur + delta).rem_euclid(len as i32) as usize;
        state.select(Some(new));
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
        self.queue.clear();
        self.queue_state.select(None);
        self.queue_index = None;
        self.player.stop();
    }

    fn play_current(&mut self) {
        if let Some(i) = self.queue_index {
            if let Some(t) = self.queue.get(i).cloned() {
                match self.player.play(&t) {
                    Ok(_) => self.status = format!("Playing: {}", t.display()),
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
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let p = std::path::PathBuf::from(line);
            if p.exists() {
                self.queue.push(Track::from_path_with_meta(p));
                loaded += 1;
            }
        }
        if loaded > 0 && self.queue_state.selected().is_none() {
            self.queue_state.select(Some(0));
        }
        self.status = format!("Loaded {} tracks from {}", loaded, path.display());
    }
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

fn scan_library(dirs: &[PathBuf]) -> Vec<Track> {
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
                    out.push(Track::from_path_with_meta(path.to_path_buf()));
                }
            }
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}
