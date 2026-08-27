use ratatui::layout::Rect;
use std::{path::PathBuf, sync::Arc};

use crate::audio::Track;

pub const MAX_UNDO_SNAPSHOTS: usize = 10;
pub const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac"];

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
pub struct UndoSnapshot {
    pub queue: Vec<Track>,
    pub queue_index: Option<usize>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotifyTab {
    Search,
    MyPlaylists,
    LikedSongs,
}

#[derive(Debug, Clone)]
pub struct PlaylistEntry {
    pub name: String,
    pub path: PathBuf,
    pub track_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutRects {
    pub library: Rect,
    pub queue: Rect,
    pub progress: Rect,
    pub progress_total_ms: u64,
    pub art_area: Rect,
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
    Track(Arc<Track>),
    Dir(PathBuf),
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
pub struct LibraryViewFingerprint {
    pub library_revision: u64,
    pub history_revision: u64,
    pub view_mode: ViewMode,
    pub sort: SortMode,
    pub search: String,
}

#[derive(Debug)]
pub struct LibraryViewCache {
    pub fingerprint: LibraryViewFingerprint,
    pub rows: Vec<LibraryRow>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteCategory {
    Command,
    Theme,
    EqPreset,
    Track,
    Radio,
    View,
}

impl PaletteCategory {
    pub fn label(self) -> &'static str {
        match self {
            PaletteCategory::Command => "Comando",
            PaletteCategory::Theme => "Tema",
            PaletteCategory::EqPreset => "Equalizador",
            PaletteCategory::Track => "Música",
            PaletteCategory::Radio => "Rádio",
            PaletteCategory::View => "Visão",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            PaletteCategory::Command => "⚡",
            PaletteCategory::Theme => "🎨",
            PaletteCategory::EqPreset => "🎚️",
            PaletteCategory::Track => "🎵",
            PaletteCategory::Radio => "📻",
            PaletteCategory::View => "👁️",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    Execute(crate::keybinds::Action),
    SetTheme(String),
    SetEqPreset(usize),
    PlayTrack(PathBuf),
    SetViewMode(ViewMode),
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: PaletteCategory,
    pub action: PaletteAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsonicTab {
    Search,
    RecentAlbums,
    Playlists,
    Random,
}

impl SubsonicTab {
    pub fn label(self) -> &'static str {
        match self {
            SubsonicTab::Search => "Busca",
            SubsonicTab::RecentAlbums => "Álbuns Recentes",
            SubsonicTab::Playlists => "Playlists",
            SubsonicTab::Random => "Músicas Aleatórias",
        }
    }
}


