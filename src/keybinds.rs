use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::Keybinds;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    Help,
    Search,
    Tab,
    PlayPause,
    Next,
    Prev,
    Stop,
    Shuffle,
    Repeat,
    Sort,
    SleepTimer,
    SavePlaylist,
    LoadPlaylist,
    VolumeUp,
    VolumeDown,
    SeekBack,
    SeekForward,
    SelectionUp,
    SelectionDown,
    ActivateSelection,
    Enqueue,
    RemoveQueueItem,
    ClearQueue,
    SpotifyLogin,
    SpotifyToggle,
    ToggleView,
    EqLowUp,
    EqLowDown,
    EqMidUp,
    EqMidDown,
    EqHighUp,
    EqHighDown,
    OpenUrl,
    EqPreset,
    Rescan,
    TrackInfo,
    CycleTheme,
    VizSensUp,
    VizSensDown,
    UndoQueue,
    RecentlyPlayed,
    ShowAudioPanel,
    ReplayGain,
    CycleVizMode,
    ToggleFavorite,
    ToggleMini,
    LastfmLogin,
    SelectDevice,
    EqTuner,
    Profiles,
    SpotifyBrowser,
    /// Radio Mode (#56) — open seed prompt / toggle.
    RadioMode,
    /// Toggle the listening-stats modal (#64).
    ShowStats,
    /// Toggle the Last.fm dashboard (#63).
    LastfmPanel,
    /// Edit track tags / metadata in modal.
    EditTags,
    /// Open Online Radio directory & curated stations.
    RadioBrowser,
    /// Check and apply in-place app updates from GitHub.
    SelfUpdate,
    /// Switch directly to Library view.
    ViewLibrary,
    /// Switch focus directly to Queue view.
    ViewQueue,
    /// Switch directly to Dedicated Radio view.
    ViewRadio,
    /// Switch directly to File Browser view.
    ViewBrowser,
    /// Toggle the Karaoke / Synced Lyrics modal.
    ShowLyrics,
    /// Open Unified Command Palette (#123).
    CommandPalette,
    /// Open Subsonic / Navidrome Cloud Browser (#126).
    SubsonicBrowser,
    /// Open Cloud Audio Vault Browser (#122).
    VaultBrowser,
    /// Open Share & Publish Playlist Modal (#82).
    SharePlaylist,
    /// Open Browse Public Playlists Modal (#83).
    BrowsePlaylists,
    /// Toggle Endless Mode / Smart Auto-Play (#133).
    ToggleEndlessMode,
    /// Download & cache current streaming track to local disk (#134).
    DownloadCurrent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            mods: KeyModifiers::NONE,
        }
    }
    pub fn with_mods(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }
}

pub fn parse_chord(s: &str) -> Option<KeyChord> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Bare "+" / "-" are literal keys, not modifier-key syntax. Without this short
    // circuit, split('+') would shred "+" into ["", ""] and the chord would fail
    // to parse — surfacing as "invalid keybind '+'" on startup.
    if s == "+" {
        return Some(KeyChord::new(KeyCode::Char('+')));
    }
    if s == "-" {
        return Some(KeyChord::new(KeyCode::Char('-')));
    }
    let mut mods = KeyModifiers::NONE;
    let mut parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    let key = parts.pop()?;
    for m in parts {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
            "shift" => mods |= KeyModifiers::SHIFT,
            "alt" => mods |= KeyModifiers::ALT,
            _ => return None,
        }
    }
    let code = match key.to_lowercase().as_str() {
        "space" | "spc" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "backspace" | "bs" => KeyCode::Backspace,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "+" => KeyCode::Char('+'),
        "-" => KeyCode::Char('-'),
        "=" => KeyCode::Char('='),
        "_" => KeyCode::Char('_'),
        "/" => KeyCode::Char('/'),
        "?" => KeyCode::Char('?'),
        single if single.chars().count() == 1 => {
            // crossterm emits uppercase letters as (Char('A'), SHIFT). To match,
            // store the original case and infer SHIFT from uppercase input (#67).
            let ch = key.chars().next().unwrap();
            if ch.is_ascii_uppercase() {
                mods |= KeyModifiers::SHIFT;
                KeyCode::Char(ch.to_ascii_lowercase())
            } else {
                KeyCode::Char(ch)
            }
        }
        _ => return None,
    };
    Some(KeyChord { code, mods })
}

pub struct Bindings {
    table: Vec<(KeyChord, Action)>,
}

impl Bindings {
    pub fn from_config(kb: &Keybinds) -> (Self, Vec<String>) {
        // #67: every action below can be overridden in [keybinds]. Empty strings fall
        // back to the built-in defaults further down. Conflicts (two specs binding the
        // same chord) are collected in `warnings` so App can surface them at startup.
        let overrides: &[(&str, Action)] = &[
            (&kb.quit, Action::Quit),
            (&kb.play_pause, Action::PlayPause),
            (&kb.next, Action::Next),
            (&kb.prev, Action::Prev),
            (&kb.volume_up, Action::VolumeUp),
            (&kb.volume_down, Action::VolumeDown),
            (&kb.seek_back, Action::SeekBack),
            (&kb.seek_forward, Action::SeekForward),
            (&kb.stop, Action::Stop),
            (&kb.shuffle, Action::Shuffle),
            (&kb.repeat, Action::Repeat),
            (&kb.search, Action::Search),
            (&kb.tab, Action::Tab),
            (&kb.enqueue, Action::Enqueue),
            (&kb.remove_from_queue, Action::RemoveQueueItem),
            (&kb.clear_queue, Action::ClearQueue),
            (&kb.toggle_mini, Action::ToggleMini),
            (&kb.toggle_view, Action::ToggleView),
            (&kb.rescan, Action::Rescan),
            (&kb.help, Action::Help),
            (&kb.cycle_theme, Action::CycleTheme),
            (&kb.open_url, Action::OpenUrl),
            (&kb.toggle_favorite, Action::ToggleFavorite),
            (&kb.command_palette, Action::CommandPalette),
            (&kb.subsonic_browser, Action::SubsonicBrowser),
            (&kb.vault_browser, Action::VaultBrowser),
        ];
        let mut table: Vec<(KeyChord, Action)> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        for (spec, action) in overrides {
            if spec.is_empty() {
                continue;
            }
            let Some(c) = parse_chord(spec) else {
                warnings.push(format!("invalid keybind '{spec}' for {action:?}"));
                continue;
            };
            if let Some((_, existing)) = table.iter().find(|(c2, _)| *c2 == c) {
                warnings.push(format!(
                    "conflicting keybind '{spec}' for {action:?} (already mapped to {existing:?})"
                ));
                continue;
            }
            table.push((c, *action));
        }

        let builtins: &[(KeyChord, Action)] = &[
            (KeyChord::new(KeyCode::Char('?')), Action::Help),
            (KeyChord::new(KeyCode::Char('/')), Action::Search),
            (KeyChord::new(KeyCode::Tab), Action::Tab),
            (KeyChord::new(KeyCode::Char('s')), Action::Stop),
            (KeyChord::new(KeyCode::Char('S')), Action::Shuffle),
            (KeyChord::new(KeyCode::Char('r')), Action::Repeat),
            (KeyChord::new(KeyCode::Char('o')), Action::Sort),
            (KeyChord::new(KeyCode::Char('T')), Action::SleepTimer),
            (KeyChord::new(KeyCode::Char('w')), Action::SavePlaylist),
            (KeyChord::new(KeyCode::Char('L')), Action::LoadPlaylist),
            (KeyChord::new(KeyCode::Left), Action::SeekBack),
            (KeyChord::new(KeyCode::Right), Action::SeekForward),
            (KeyChord::new(KeyCode::Up), Action::SelectionUp),
            (KeyChord::new(KeyCode::Char('k')), Action::SelectionUp),
            (KeyChord::new(KeyCode::Down), Action::SelectionDown),
            (KeyChord::new(KeyCode::Char('j')), Action::SelectionDown),
            (KeyChord::new(KeyCode::Enter), Action::ActivateSelection),
            (KeyChord::new(KeyCode::Char('a')), Action::Enqueue),
            (KeyChord::new(KeyCode::Char('d')), Action::RemoveQueueItem),
            (KeyChord::new(KeyCode::Char('c')), Action::ClearQueue),
            (KeyChord::new(KeyCode::Char('P')), Action::SpotifyLogin),
            (KeyChord::new(KeyCode::Char('@')), Action::SpotifyToggle),
            (KeyChord::new(KeyCode::Char('V')), Action::ToggleView),
            (KeyChord::new(KeyCode::Char('1')), Action::ViewLibrary),
            (KeyChord::new(KeyCode::Char('2')), Action::ViewQueue),
            (KeyChord::new(KeyCode::Char('3')), Action::ViewRadio),
            (KeyChord::new(KeyCode::Char('4')), Action::ViewBrowser),
            (KeyChord::new(KeyCode::Char('i')), Action::OpenUrl),
            (KeyChord::new(KeyCode::Char('0')), Action::EqPreset),
            (KeyChord::new(KeyCode::Char('E')), Action::EqPreset),
            (KeyChord::new(KeyCode::Char('R')), Action::Rescan),
            (KeyChord::new(KeyCode::Char('I')), Action::TrackInfo),
            (KeyChord::new(KeyCode::BackTab), Action::CycleTheme),
            (KeyChord::new(KeyCode::Char('[')), Action::VizSensDown),
            (KeyChord::new(KeyCode::Char(']')), Action::VizSensUp),
            (KeyChord::new(KeyCode::Char('u')), Action::UndoQueue),
            (
                KeyChord::with_mods(KeyCode::Char('z'), KeyModifiers::CONTROL),
                Action::UndoQueue,
            ),
            (KeyChord::new(KeyCode::Char('H')), Action::RecentlyPlayed),
            (KeyChord::new(KeyCode::Char('e')), Action::ShowAudioPanel),
            (KeyChord::new(KeyCode::Char('G')), Action::ReplayGain),
            (KeyChord::new(KeyCode::Char('v')), Action::CycleVizMode),
            (KeyChord::new(KeyCode::Char('f')), Action::ToggleFavorite),
            (KeyChord::new(KeyCode::Char('m')), Action::ToggleMini),
            (KeyChord::new(KeyCode::Char('F')), Action::LastfmLogin),
            (KeyChord::new(KeyCode::Char('D')), Action::SelectDevice),
            (KeyChord::new(KeyCode::Char('E')), Action::EqTuner),
            (KeyChord::new(KeyCode::Char('O')), Action::Profiles),
            (KeyChord::new(KeyCode::Char('B')), Action::SpotifyBrowser),
            (KeyChord::new(KeyCode::Char('T')), Action::EditTags),
            (KeyChord::new(KeyCode::Char('K')), Action::RadioBrowser),
            (KeyChord::new(KeyCode::Char('y')), Action::ShowLyrics),
            (KeyChord::new(KeyCode::Char('Y')), Action::ShowLyrics),
            (KeyChord::new(KeyCode::Char('U')), Action::SelfUpdate),
            (
                KeyChord::with_mods(KeyCode::Char('r'), KeyModifiers::CONTROL),
                Action::RadioMode,
            ),
            (
                KeyChord::with_mods(KeyCode::Char('s'), KeyModifiers::CONTROL),
                Action::ShowStats,
            ),
            (
                KeyChord::with_mods(KeyCode::Char('l'), KeyModifiers::CONTROL),
                Action::LastfmPanel,
            ),
            (
                KeyChord::with_mods(KeyCode::Char('p'), KeyModifiers::CONTROL),
                Action::CommandPalette,
            ),
            (
                KeyChord::with_mods(KeyCode::Char('n'), KeyModifiers::CONTROL),
                Action::SubsonicBrowser,
            ),
            (KeyChord::new(KeyCode::Char('X')), Action::SharePlaylist),
            (KeyChord::new(KeyCode::Char('C')), Action::BrowsePlaylists),
        ];
        for (c, a) in builtins {
            if !table.iter().any(|(c2, _)| c2 == c) {
                table.push((*c, *a));
            }
        }

        (Self { table }, warnings)
    }

    pub fn lookup(&self, code: KeyCode, mods: KeyModifiers) -> Option<Action> {
        let chord = KeyChord::with_mods(code, mods);
        let chord_no_shift = KeyChord::with_mods(code, mods - KeyModifiers::SHIFT);
        self.table
            .iter()
            .find(|(c, _)| *c == chord)
            .map(|(_, a)| *a)
            .or_else(|| {
                self.table
                    .iter()
                    .find(|(c, _)| *c == chord_no_shift)
                    .map(|(_, a)| *a)
            })
    }

    pub fn resolve(&self, key: &crossterm::event::KeyEvent) -> Option<Action> {
        self.lookup(key.code, key.modifiers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_chord_plain_letter() {
        let c = parse_chord("a").unwrap();
        assert_eq!(c.code, KeyCode::Char('a'));
        assert_eq!(c.mods, KeyModifiers::NONE);
    }

    #[test]
    fn parse_chord_with_ctrl() {
        let c = parse_chord("Ctrl+p").unwrap();
        assert_eq!(c.code, KeyCode::Char('p'));
        assert!(c.mods.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_chord_with_shift() {
        let c = parse_chord("Shift+s").unwrap();
        assert!(c.mods.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn parse_chord_combined_modifiers() {
        let c = parse_chord("Ctrl+Shift+x").unwrap();
        assert!(c.mods.contains(KeyModifiers::CONTROL));
        assert!(c.mods.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn parse_chord_named_keys() {
        assert_eq!(parse_chord("Tab").unwrap().code, KeyCode::Tab);
        assert_eq!(parse_chord("Enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_chord("Esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_chord("Up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_chord("space").unwrap().code, KeyCode::Char(' '));
    }

    #[test]
    fn parse_chord_literal_plus_and_minus() {
        // Bare "+" / "-" must parse as literal keys, not split-on-+ noise.
        assert_eq!(parse_chord("+").unwrap().code, KeyCode::Char('+'));
        assert_eq!(parse_chord("-").unwrap().code, KeyCode::Char('-'));
    }

    #[test]
    fn parse_chord_rejects_invalid() {
        assert!(parse_chord("").is_none());
        assert!(parse_chord("Shift+Foo").is_none());
        assert!(parse_chord("NotARealKey").is_none());
    }

    #[test]
    fn parse_chord_trailing_plus_is_invalid() {
        // "Ctrl+" splits into ["Ctrl", ""] — empty key has no valid mapping.
        assert!(parse_chord("Ctrl+").is_none());
    }

    #[test]
    fn command_palette_default_lookup() {
        let (bindings, _) = Bindings::from_config(&Keybinds {
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
        });
        assert_eq!(
            bindings.lookup(KeyCode::Char('p'), KeyModifiers::CONTROL),
            Some(Action::CommandPalette)
        );
        assert_eq!(
            bindings.lookup(KeyCode::Char('n'), KeyModifiers::CONTROL),
            Some(Action::SubsonicBrowser)
        );
    }
}
