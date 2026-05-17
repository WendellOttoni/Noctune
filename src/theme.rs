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
        fs::create_dir_all(&dir).ok();
        let path = dir.join(format!("{name}.toml"));
        if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("reading theme {}", path.display()))?;
            Ok(toml::from_str(&text)?)
        } else {
            let theme = Theme::default();
            fs::write(&path, toml::to_string_pretty(&theme)?).ok();
            Ok(theme)
        }
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

pub fn parse_color(s: &str) -> Color {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
    }
    match s.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "white" => Color::White,
        _ => Color::Reset,
    }
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
