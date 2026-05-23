use anyhow::{Context, Result};
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: Colors,
    pub ascii: AsciiArt,
    pub symbols: Symbols,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colors {
    pub background: String,
    pub foreground: String,
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub muted: String,
    pub border: String,
    pub border_focused: String,
    pub progress_filled: String,
    pub progress_empty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciiArt {
    pub logo: String,
    pub playing: String,
    pub paused: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbols {
    pub play: String,
    pub pause: String,
    pub stop: String,
    pub next: String,
    pub prev: String,
    pub shuffle: String,
    pub repeat: String,
    pub volume: String,
    pub progress_fill: String,
    pub progress_empty: String,
    pub progress_head: String,
}

impl Theme {
    pub fn load(name: &str) -> Result<Self> {
        let dir = config::themes_dir()?;
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("theme: failed to create dir {}: {e}", dir.display());
        }
        let path = dir.join(format!("{name}.toml"));
        let theme: Self = if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("reading theme {}", path.display()))?;
            toml::from_str(&text)?
        } else {
            let theme = Theme::default();
            if let Err(e) = fs::write(&path, toml::to_string_pretty(&theme)?) {
                eprintln!("theme: failed to write default {}: {e}", path.display());
            }
            theme
        };
        // #103: validate color fields once on load and emit aggregated warnings to
        // stderr (full logging via tracing comes with #96). Invalid colors still
        // fall back to Color::Reset at render time — same behaviour as before, just
        // no longer silent.
        let issues = theme.validate_colors();
        if !issues.is_empty() {
            eprintln!(
                "theme '{}': {} invalid color(s) — using defaults",
                theme.name,
                issues.len()
            );
            for (field, err) in &issues {
                eprintln!("  {field}: {err}");
            }
        }
        Ok(theme)
    }

    /// Returns a list of `(field_name, error)` for every color field that fails
    /// to parse. Empty vec means the theme is fully valid.
    pub fn validate_colors(&self) -> Vec<(&'static str, ColorParseError)> {
        let fields: [(&'static str, &str); 10] = [
            ("background", &self.colors.background),
            ("foreground", &self.colors.foreground),
            ("primary", &self.colors.primary),
            ("secondary", &self.colors.secondary),
            ("accent", &self.colors.accent),
            ("muted", &self.colors.muted),
            ("border", &self.colors.border),
            ("border_focused", &self.colors.border_focused),
            ("progress_filled", &self.colors.progress_filled),
            ("progress_empty", &self.colors.progress_empty),
        ];
        fields
            .into_iter()
            .filter_map(|(name, value)| try_parse_color(value).err().map(|e| (name, e)))
            .collect()
    }

    #[allow(dead_code)]
    pub fn style(&self, color: &str) -> Style {
        Style::default().fg(parse_color(color))
    }

    pub fn border(&self, focused: bool) -> Style {
        let c = if focused {
            &self.colors.border_focused
        } else {
            &self.colors.border
        };
        Style::default().fg(parse_color(c))
    }

    pub fn accent(&self) -> Style {
        Style::default()
            .fg(parse_color(&self.colors.accent))
            .add_modifier(Modifier::BOLD)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".into(),
            colors: Colors {
                background: "#0b0b1a".into(),
                foreground: "#e6e6f0".into(),
                primary: "#9d7cf2".into(),
                secondary: "#5fb3d8".into(),
                accent: "#f0b67f".into(),
                muted: "#6a6a85".into(),
                border: "#2a2a44".into(),
                border_focused: "#9d7cf2".into(),
                progress_filled: "#9d7cf2".into(),
                progress_empty: "#2a2a44".into(),
            },
            ascii: AsciiArt {
                logo: DEFAULT_LOGO.into(),
                playing: DEFAULT_PLAYING.into(),
                paused: DEFAULT_PAUSED.into(),
            },
            symbols: Symbols {
                play: "▶".into(),
                pause: "⏸".into(),
                stop: "■".into(),
                next: "⏭".into(),
                prev: "⏮".into(),
                shuffle: "⇄".into(),
                repeat: "↻".into(),
                volume: "♪".into(),
                progress_fill: "━".into(),
                progress_empty: "─".into(),
                progress_head: "●".into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorParseError {
    /// Starts with `#` but is not a valid 6-digit hex code.
    InvalidHex(String),
    /// Bare name that does not match any known palette entry.
    UnknownName(String),
    /// Empty / whitespace-only value.
    Empty,
}

impl std::fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHex(s) => write!(f, "invalid hex color '{s}' (expected #RRGGBB)"),
            Self::UnknownName(s) => write!(f, "unknown color name '{s}'"),
            Self::Empty => write!(f, "empty color value"),
        }
    }
}

impl std::error::Error for ColorParseError {}

/// Strict color parser. Returns an error describing why the input is not a
/// recognised color, so callers (e.g. `Theme::validate_colors`) can surface it.
pub fn try_parse_color(s: &str) -> Result<Color, ColorParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ColorParseError::Empty);
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Ok(Color::Rgb(r, g, b));
            }
        }
        return Err(ColorParseError::InvalidHex(s.to_string()));
    }
    match s.to_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "white" => Ok(Color::White),
        _ => Err(ColorParseError::UnknownName(s.to_string())),
    }
}

/// Lenient parser used on the render hot path: falls back to `Color::Reset` on
/// any parse error. Validation/warnings happen once in `Theme::load`, so render
/// frames stay allocation-free.
pub fn parse_color(s: &str) -> Color {
    try_parse_color(s).unwrap_or(Color::Reset)
}

const DEFAULT_LOGO: &str = r#"
███╗   ██╗ ██████╗  ██████╗████████╗██╗   ██╗███╗   ██╗███████╗
████╗  ██║██╔═══██╗██╔════╝╚══██╔══╝██║   ██║████╗  ██║██╔════╝
██╔██╗ ██║██║   ██║██║        ██║   ██║   ██║██╔██╗ ██║█████╗
██║╚██╗██║██║   ██║██║        ██║   ██║   ██║██║╚██╗██║██╔══╝
██║ ╚████║╚██████╔╝╚██████╗   ██║   ╚██████╔╝██║ ╚████║███████╗
╚═╝  ╚═══╝ ╚═════╝  ╚═════╝   ╚═╝    ╚═════╝ ╚═╝  ╚═══╝╚══════╝
"#;

const DEFAULT_PLAYING: &str = r#"
   ♪  ♫  ♪  ♫
  ╱╲╱╲╱╲╱╲╱╲
 ╱  ▶ NOW  ╲
╱  PLAYING  ╲
"#;

const DEFAULT_PAUSED: &str = r#"
   ·  ·  ·  ·
  ┄┄┄┄┄┄┄┄┄┄
   ⏸ PAUSED
  ┄┄┄┄┄┄┄┄┄┄
"#;
