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
        single if single.chars().count() == 1 => KeyCode::Char(single.chars().next().unwrap()),
        _ => return None,
    };
    Some(KeyChord { code, mods })
}

pub struct Bindings {
    table: Vec<(KeyChord, Action)>,
}

impl Bindings {
    pub fn from_config(kb: &Keybinds) -> Self {
        let defaults: &[(&str, Action)] = &[
            (&kb.quit, Action::Quit),
            (&kb.play_pause, Action::PlayPause),
            (&kb.next, Action::Next),
            (&kb.prev, Action::Prev),
            (&kb.volume_up, Action::VolumeUp),
            (&kb.volume_down, Action::VolumeDown),
        ];
        let mut table: Vec<(KeyChord, Action)> = Vec::new();
        for (spec, action) in defaults {
            if let Some(c) = parse_chord(spec) {
                table.push((c, *action));
            }
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
            (KeyChord::new(KeyCode::Char('1')), Action::EqLowDown),
            (KeyChord::new(KeyCode::Char('2')), Action::EqLowUp),
            (KeyChord::new(KeyCode::Char('3')), Action::EqMidDown),
            (KeyChord::new(KeyCode::Char('4')), Action::EqMidUp),
            (KeyChord::new(KeyCode::Char('5')), Action::EqHighDown),
            (KeyChord::new(KeyCode::Char('6')), Action::EqHighUp),
            (KeyChord::new(KeyCode::Char('i')), Action::OpenUrl),
            (KeyChord::new(KeyCode::Char('0')), Action::EqPreset),
            (KeyChord::new(KeyCode::Char('R')), Action::Rescan),
            (KeyChord::new(KeyCode::Char('I')), Action::TrackInfo),
            (KeyChord::new(KeyCode::BackTab), Action::CycleTheme),
            (KeyChord::new(KeyCode::Char('[')), Action::VizSensDown),
            (KeyChord::new(KeyCode::Char(']')), Action::VizSensUp),
            (KeyChord::new(KeyCode::Char('u')), Action::UndoQueue),
            (KeyChord::with_mods(KeyCode::Char('z'), KeyModifiers::CONTROL), Action::UndoQueue),
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
        ];
        for (c, a) in builtins {
            if !table.iter().any(|(c2, _)| c2 == c) {
                table.push((*c, *a));
            }
        }

        Self { table }
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
}
