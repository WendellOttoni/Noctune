use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    app::{App, StatusKind},
    theme::{parse_color, Theme},
};

pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Format a unix timestamp as a short relative duration ago — e.g. "5m", "3h",
/// "2d", "3w". `0` (never played) returns "never" so callers can branch on it.
/// Used by the playlist browser row hints (#84).
pub fn relative_time(ts: u64) -> String {
    if ts == 0 {
        return "never".into();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now.saturating_sub(ts);
    if secs < 60 {
        return "just now".into();
    }
    let m = secs / 60;
    if m < 60 {
        return format!("{m}m ago");
    }
    let h = m / 60;
    if h < 24 {
        return format!("{h}h ago");
    }
    let d = h / 24;
    if d < 14 {
        return format!("{d}d ago");
    }
    let w = d / 7;
    format!("{w}w ago")
}

/// `" [done/total]"` if a scan is reporting progress, else empty (#104).
pub fn scan_progress_suffix(app: &App) -> String {
    match app.scan_progress {
        Some((d, t)) if t > 0 => format!(" [{d}/{t}]"),
        _ => String::new(),
    }
}

/// Pick the status-bar foreground color from the current `StatusKind` (#102).
/// Error → red, Warning → yellow, Info → theme.secondary.
pub fn status_style(app: &App) -> Style {
    match app.status_kind {
        StatusKind::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        StatusKind::Warning => Style::default().fg(Color::Yellow),
        StatusKind::Info => Style::default().fg(parse_color(&app.theme.colors.secondary)),
    }
}

pub fn build_eq_row(eq: &crate::eq::EqState, theme: &Theme) -> Line<'static> {
    let muted = parse_color(&theme.colors.muted);
    let accent = parse_color(&theme.colors.accent);
    let primary = parse_color(&theme.colors.primary);

    let band = |label: &str, db: f32| -> Vec<Span<'static>> {
        let color = if db == 0.0 {
            muted
        } else if db > 0.0 {
            accent
        } else {
            primary
        };
        vec![
            Span::styled(format!("{}: ", label), Style::default().fg(muted)),
            Span::styled(format!("{:+.0}dB", db), Style::default().fg(color)),
            Span::raw("  "),
        ]
    };

    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        " ≡ EQ  ".to_string(),
        Style::default().fg(muted),
    )];
    spans.extend(band("L", eq.low_db()));
    spans.extend(band("M", eq.mid_db()));
    spans.extend(band("H", eq.high_db()));
    Line::from(spans)
}

pub fn build_volume_bar(volume: f32, width: usize, theme: &Theme) -> Line<'static> {
    let pct = (volume * 100.0).round() as u32;
    let frac = (volume / 1.5).clamp(0.0, 1.0);
    let filled = ((width as f32) * frac).round() as usize;
    let empty = width.saturating_sub(filled);
    Line::from(vec![
        Span::styled(
            format!(" {} ", theme.symbols.volume),
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            "█".repeat(filled),
            Style::default().fg(parse_color(&theme.colors.primary)),
        ),
        Span::styled(
            "░".repeat(empty),
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled(
            format!(" {pct}%"),
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
    ])
}

/// Split `text` into spans, applying `match_color` to occurrences of `needle`
/// (case-insensitive) and `base_color` to the rest. Used by the search filter to
/// highlight matched substrings (#66).
pub fn highlight_match(text: &str, needle: &str, base: Color, hit: Color) -> Vec<Span<'static>> {
    let lower = text.to_lowercase();
    let needle_l = needle.to_lowercase();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut cursor = 0;
    while let Some(rel) = lower[cursor..].find(&needle_l) {
        let abs = cursor + rel;
        if abs > cursor {
            out.push(Span::styled(
                text[cursor..abs].to_string(),
                Style::default().fg(base),
            ));
        }
        let end = abs + needle_l.len();
        out.push(Span::styled(
            text[abs..end].to_string(),
            Style::default().fg(hit).add_modifier(Modifier::BOLD),
        ));
        cursor = end;
    }
    if cursor < text.len() {
        out.push(Span::styled(
            text[cursor..].to_string(),
            Style::default().fg(base),
        ));
    }
    if out.is_empty() {
        out.push(Span::styled(text.to_string(), Style::default().fg(base)));
    }
    out
}

pub fn build_progress(
    elapsed: Duration,
    total: Option<Duration>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let empty_color = Style::default().fg(parse_color(&theme.colors.progress_empty));
    let filled_color = Style::default().fg(parse_color(&theme.colors.progress_filled));
    let head_color = Style::default().fg(parse_color(&theme.colors.accent));

    // Issue #61: when total duration is unknown (e.g. live stream, M3U URL with no
    // metadata), render an indeterminate bar instead of pretending the song is 240s
    // long. A small marker scrolls left-to-right based on elapsed time so the user
    // still gets visual feedback that audio is flowing.
    let Some(total) = total else {
        if width == 0 {
            return Line::from("");
        }
        let pos = (elapsed.as_secs() as usize) % width.max(1);
        let before = theme.symbols.progress_empty.repeat(pos);
        let after = theme
            .symbols
            .progress_empty
            .repeat(width.saturating_sub(pos + 1));
        return Line::from(vec![
            Span::raw(" "),
            Span::styled(before, empty_color),
            Span::styled(theme.symbols.progress_head.clone(), head_color),
            Span::styled(after, empty_color),
        ]);
    };

    let frac = if total.is_zero() {
        0.0
    } else {
        (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
    };
    let filled = ((width as f32) * frac).round() as usize;
    let empty = width.saturating_sub(filled);

    let (fill, head, empt) = if filled == 0 {
        (
            String::new(),
            String::new(),
            theme.symbols.progress_empty.repeat(empty),
        )
    } else if filled >= width {
        (
            theme.symbols.progress_fill.repeat(width),
            String::new(),
            String::new(),
        )
    } else {
        (
            theme.symbols.progress_fill.repeat(filled.saturating_sub(1)),
            theme.symbols.progress_head.clone(),
            theme.symbols.progress_empty.repeat(empty),
        )
    };

    Line::from(vec![
        Span::raw(" "),
        Span::styled(fill, filled_color),
        Span::styled(head, head_color),
        Span::styled(empt, empty_color),
    ])
}

pub fn inner_rect(r: Rect) -> Rect {
    Rect {
        x: r.x + 1,
        y: r.y + 1,
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    }
}
