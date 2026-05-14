use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;

use crate::{
    app::{App, Pane},
    theme::{parse_color, Theme},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, chunks[0], &app.theme);
    render_main(f, chunks[1], app);
    render_visualizer(f, chunks[2], app);
    render_now_playing(f, chunks[3], app);
    render_status(f, chunks[4], app);

    if app.show_help {
        render_help(f, area, &app.theme);
    }
}

fn render_help(f: &mut Frame, area: Rect, theme: &Theme) {
    let w = 56.min(area.width.saturating_sub(4));
    let h = 22.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let lines: Vec<Line> = vec![
        Line::from("  Playback"),
        Line::from("    Space      play / pause"),
        Line::from("    n / p      next / previous"),
        Line::from("    s          stop"),
        Line::from("    ← / →      seek -5s / +5s"),
        Line::from("    + / -      volume up / down"),
        Line::from(""),
        Line::from("  Library / Queue"),
        Line::from("    Tab        switch focus"),
        Line::from("    ↑↓ / jk    move selection"),
        Line::from("    Enter      play"),
        Line::from("    a / d      add / remove from queue"),
        Line::from("    c          clear queue"),
        Line::from("    /          search library"),
        Line::from(""),
        Line::from("  Modes & playlists"),
        Line::from("    Shift+S    toggle shuffle"),
        Line::from("    r          cycle repeat (off/all/one)"),
        Line::from("    w          save queue as .m3u"),
        Line::from("    Shift+L    load latest .m3u"),
    ];

    let p = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(parse_color(&theme.colors.foreground)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(" Help — press any key ", theme.accent())),
        );
    f.render_widget(p, popup);
}

fn render_visualizer(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(false))
        .title(Span::styled(" Spectrum ", app.theme.accent()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    let bar_w: u16 = 2;
    let gap: u16 = 1;
    let stride = bar_w + gap;
    let n_bars = ((inner.width as u16) / stride).max(1) as usize;
    let bars = app.tap.compute_bars(n_bars);

    let blocks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let h = inner.height as usize;
    let primary = parse_color(&app.theme.colors.primary);
    let secondary = parse_color(&app.theme.colors.secondary);
    let accent = parse_color(&app.theme.colors.accent);

    for row in 0..h {
        let mut spans: Vec<Span> = Vec::with_capacity(n_bars * 2);
        let row_from_bottom = h - 1 - row;
        for (i, &val) in bars.iter().enumerate() {
            let bar_units = (val * (h * blocks.len()) as f32) as usize;
            let full_rows = bar_units / blocks.len();
            let rem = bar_units % blocks.len();

            let ch = if row_from_bottom < full_rows {
                '█'
            } else if row_from_bottom == full_rows && rem > 0 {
                blocks[rem.saturating_sub(1)]
            } else {
                ' '
            };

            let color = if val > 0.75 {
                accent
            } else if val > 0.4 {
                primary
            } else {
                secondary
            };

            let bar_text: String = std::iter::repeat(ch).take(bar_w as usize).collect();
            spans.push(Span::styled(bar_text, Style::default().fg(color)));
            if i + 1 < n_bars {
                spans.push(Span::raw(" ".repeat(gap as usize)));
            }
            let _ = i;
        }
        let line = Line::from(spans);
        let r = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), r);
    }
}

fn render_header(f: &mut Frame, area: Rect, theme: &Theme) {
    let lines: Vec<Line> = theme
        .ascii
        .logo
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                l.to_string(),
                Style::default()
                    .fg(parse_color(&theme.colors.primary))
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    let p = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(theme.border(false)),
        );
    f.render_widget(p, area);
}

fn render_main(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_library(f, chunks[0], app);
    render_queue(f, chunks[1], app);
}

fn render_library(f: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Pane::Library;
    let items: Vec<ListItem> = app
        .visible_library()
        .into_iter()
        .map(|t| {
            ListItem::new(Line::from(Span::styled(
                t.display(),
                Style::default().fg(parse_color(&app.theme.colors.foreground)),
            )))
        })
        .collect();

    let total = app.library.len();
    let shown = items.len();
    let title = if app.search_active() || !app.search_query().is_empty() {
        format!(" Library [{}/{}] /{} ", shown, total, app.search_query())
    } else {
        format!(" Library ({}) ", total)
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border(focused))
                .title(Span::styled(title, app.theme.accent())),
        )
        .highlight_style(
            Style::default()
                .bg(parse_color(&app.theme.colors.primary))
                .fg(parse_color(&app.theme.colors.background))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ▶ ");

    f.render_stateful_widget(list, area, &mut app.library_state);
}

fn render_queue(f: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Pane::Queue;
    let current = app.queue_index;

    let items: Vec<ListItem> = app
        .queue
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_current = Some(i) == current;
            let style = if is_current {
                Style::default()
                    .fg(parse_color(&app.theme.colors.accent))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(parse_color(&app.theme.colors.foreground))
            };
            let prefix = if is_current {
                format!("{} ", app.theme.symbols.play)
            } else {
                "  ".to_string()
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(t.display(), style),
            ]))
        })
        .collect();

    let title = format!(" Queue ({}) ", app.queue.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border(focused))
                .title(Span::styled(title, app.theme.accent())),
        )
        .highlight_style(
            Style::default()
                .bg(parse_color(&app.theme.colors.secondary))
                .fg(parse_color(&app.theme.colors.background)),
        )
        .highlight_symbol(" ▶ ");

    f.render_stateful_widget(list, area, &mut app.queue_state);
}

fn render_now_playing(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(false))
        .title(Span::styled(" Now Playing ", app.theme.accent()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let title = app
        .player
        .current()
        .map(|t| t.display())
        .unwrap_or_else(|| "— nothing playing —".into());

    let state_sym = if app.player.current().is_none() {
        app.theme.symbols.stop.clone()
    } else if app.player.is_paused() {
        app.theme.symbols.pause.clone()
    } else {
        app.theme.symbols.play.clone()
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {state_sym}  "),
            Style::default().fg(parse_color(&app.theme.colors.accent)),
        ),
        Span::styled(
            title,
            Style::default()
                .fg(parse_color(&app.theme.colors.foreground))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    f.render_widget(header, chunks[0]);

    let elapsed = app.player.elapsed();
    let total = app.player.current().and_then(|t| t.duration);
    let progress_line = build_progress(
        elapsed,
        total,
        chunks[1].width.saturating_sub(2) as usize,
        &app.theme,
    );
    f.render_widget(Paragraph::new(progress_line), chunks[1]);

    let total_str = total
        .map(format_duration)
        .unwrap_or_else(|| "--:--".to_string());
    let time = Paragraph::new(Line::from(vec![Span::styled(
        format!(" {} / {}", format_duration(elapsed), total_str),
        Style::default().fg(parse_color(&app.theme.colors.muted)),
    )]));
    f.render_widget(time, chunks[2]);

    let vol_pct = (app.player.volume() * 100.0).round() as u32;
    let vol = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", app.theme.symbols.volume),
            Style::default().fg(parse_color(&app.theme.colors.accent)),
        ),
        Span::styled(
            format!("vol {vol_pct}%"),
            Style::default().fg(parse_color(&app.theme.colors.muted)),
        ),
    ]));
    f.render_widget(vol, chunks[3]);
}

fn build_progress(
    elapsed: Duration,
    total: Option<Duration>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let total = total.unwrap_or_else(|| Duration::from_secs(240));
    let frac = if total.is_zero() {
        0.0
    } else {
        (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
    };
    let filled = ((width as f32) * frac).round() as usize;
    let empty = width.saturating_sub(filled);

    let fill = theme.symbols.progress_fill.repeat(filled);
    let empt = theme.symbols.progress_empty.repeat(empty);

    Line::from(vec![
        Span::styled(
            " ".to_string(),
            Style::default(),
        ),
        Span::styled(
            fill,
            Style::default().fg(parse_color(&theme.colors.progress_filled)),
        ),
        Span::styled(
            empt,
            Style::default().fg(parse_color(&theme.colors.progress_empty)),
        ),
    ])
}

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let status_text = if app.search_active() {
        format!(" /{}", app.search_query())
    } else {
        format!(" {}", app.status)
    };
    let status = Paragraph::new(Span::styled(
        status_text,
        Style::default().fg(parse_color(&app.theme.colors.secondary)),
    ))
    .wrap(Wrap { trim: true });
    f.render_widget(status, chunks[0]);

    let shuf = if app.shuffle { "shuf " } else { "" };
    let rep = match app.repeat {
        crate::app::RepeatMode::Off => "",
        crate::app::RepeatMode::All => "rep:all ",
        crate::app::RepeatMode::One => "rep:one ",
    };
    let hints = Paragraph::new(Span::styled(
        format!("{shuf}{rep}[?] help [q] quit "),
        Style::default().fg(parse_color(&app.theme.colors.muted)),
    ))
    .alignment(Alignment::Right);
    f.render_widget(hints, chunks[1]);
}

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}
