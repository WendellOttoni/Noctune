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
            Constraint::Length(8),
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
    if app.show_info {
        render_track_info(f, area, app);
    }
}

fn render_track_info(f: &mut Frame, area: Rect, app: &App) {
    let Some(track) = app.player.current() else {
        return;
    };
    let meta = crate::metadata::probe_full(&track.path);

    let row = |label: &str, value: String| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("  {:<14}", label),
                Style::default().fg(parse_color(&app.theme.colors.muted)),
            ),
            Span::styled(value, Style::default().fg(parse_color(&app.theme.colors.foreground))),
        ])
    };
    let dash = "—".to_string();

    let dur_str = meta.duration
        .map(|d| {
            let s = d.as_secs();
            format!("{:02}:{:02}", s / 60, s % 60)
        })
        .unwrap_or_else(|| dash.clone());

    let rate_str = meta.sample_rate
        .map(|r| format!("{} Hz", r))
        .unwrap_or_else(|| dash.clone());
    let chans_str = meta.channels
        .map(|c| format!("{}", c))
        .unwrap_or_else(|| dash.clone());
    let bits_str = meta.bits_per_sample
        .map(|b| format!("{} bit", b))
        .unwrap_or_else(|| dash.clone());

    let lines: Vec<Line> = vec![
        row("Title",        meta.title.unwrap_or_else(|| track.title.clone())),
        row("Artist",       meta.artist.unwrap_or_else(|| dash.clone())),
        row("Album Artist", meta.album_artist.unwrap_or_else(|| dash.clone())),
        row("Album",        meta.album.unwrap_or_else(|| dash.clone())),
        row("Year",         meta.year.unwrap_or_else(|| dash.clone())),
        row("Genre",        meta.genre.unwrap_or_else(|| dash.clone())),
        row("Track #",      meta.track_number.unwrap_or_else(|| dash.clone())),
        Line::from(""),
        row("Duration",     dur_str),
        row("Codec",        meta.codec.unwrap_or_else(|| dash.clone())),
        row("Sample rate",  rate_str),
        row("Channels",     chans_str),
        row("Bit depth",    bits_str),
        Line::from(""),
        row("Path",         track.path.display().to_string()),
    ];

    let w = 72.min(area.width.saturating_sub(4));
    let h = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);
    let p = Paragraph::new(Text::from(lines)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.border(true))
            .title(Span::styled(" Track info — press any key ", app.theme.accent())),
    );
    f.render_widget(p, popup);
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
        Line::from("    c          clear queue (confirm twice)"),
        Line::from("    u          undo last clear / remove"),
        Line::from("    /          search (title, artist, album, genre, year)"),
        Line::from("    Shift+H    toggle recently played view"),
        Line::from(""),
        Line::from("  Modes & playlists"),
        Line::from("    Shift+S    toggle shuffle"),
        Line::from("    r          cycle repeat (off/all/one)"),
        Line::from("    o          cycle sort (title/artist/album)"),
        Line::from("    Shift+T    sleep timer (30 min)"),
        Line::from("    w          save queue as .m3u"),
        Line::from("    Shift+L    load latest .m3u"),
        Line::from(""),
        Line::from("  Spotify (set client_id in config first)"),
        Line::from("    Shift+P    open browser login (OAuth PKCE)"),
        Line::from("    @          toggle play/pause on Spotify"),
        Line::from(""),
        Line::from("  View & mouse"),
        Line::from("    Shift+V    toggle flat / album view"),
        Line::from("    Click      play library / queue row"),
        Line::from("    Click bar  seek to position"),
        Line::from(""),
        Line::from("  EQ (-12..+12 dB)"),
        Line::from("    1 / 2      low  -/+"),
        Line::from("    3 / 4      mid  -/+"),
        Line::from("    5 / 6      high -/+"),
        Line::from("    0          cycle preset (Flat/Bass/Vocal/Treble/Loud)"),
        Line::from(""),
        Line::from("  Library"),
        Line::from("    Shift+R    rescan music_dirs"),
        Line::from("    Shift+I    track info popup"),
        Line::from("    Shift+Tab  cycle themes"),
        Line::from(""),
        Line::from("  Online"),
        Line::from("    i          open URL (YouTube, YT Music, Spotify)"),
        Line::from(""),
        Line::from("  Visualizer"),
        Line::from("    [ / ]      sensitivity -/+ (×0.1..×3.0)"),
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
    let title = format!(" Spectrum (×{:.1}) ", app.tap.sensitivity());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(false))
        .title(Span::styled(title, app.theme.accent()));
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
    let rows = app.library_rows();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            crate::app::LibraryRow::Track(t) => {
                let dur = t
                    .duration
                    .map(|d| {
                        let s = d.as_secs();
                        format!("  {:02}:{:02}", s / 60, s % 60)
                    })
                    .unwrap_or_default();
                ListItem::new(Line::from(vec![
                    Span::styled(
                        t.display(),
                        Style::default().fg(parse_color(&app.theme.colors.foreground)),
                    ),
                    Span::styled(dur, Style::default().fg(parse_color(&app.theme.colors.muted))),
                ]))
            }
            crate::app::LibraryRow::Header(album) => ListItem::new(Line::from(Span::styled(
                format!("── {} ──", album),
                Style::default()
                    .fg(parse_color(&app.theme.colors.accent))
                    .add_modifier(Modifier::BOLD),
            ))),
        })
        .collect();

    let shown = rows
        .iter()
        .filter(|r| matches!(r, crate::app::LibraryRow::Track(_)))
        .count();
    let title = if app.view_mode == crate::app::ViewMode::RecentlyPlayed {
        if app.search_active() || !app.search_query().is_empty() {
            format!(" Recently Played [{}/{}] /{} ", shown, app.history.len(), app.search_query())
        } else {
            format!(" Recently Played ({}) ", app.history.len())
        }
    } else if app.search_active() || !app.search_query().is_empty() {
        format!(" Library [{}/{}] /{} ", shown, app.library.len(), app.search_query())
    } else {
        format!(" Library ({}) ", app.library.len())
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border(focused))
                .title(Span::styled(title.clone(), app.theme.accent())),
        )
        .highlight_style(
            Style::default()
                .bg(parse_color(&app.theme.colors.primary))
                .fg(parse_color(&app.theme.colors.background))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ▶ ");

    app.layout.library = inner_rect(area);
    if rows.is_empty() {
        let msg = if !app.search_query().is_empty() {
            format!(" No matches for /{}\n\n Press Esc to clear the search.", app.search_query())
        } else if app.view_mode == crate::app::ViewMode::RecentlyPlayed {
            " No recently played tracks.\n\n Play some tracks and they will appear here.\n Press Shift+H to return to the library.".to_string()
        } else {
            " Library is empty.\n\n Add paths to music_dirs in your config.toml\n or press Shift+R to rescan.\n Press i to paste a YouTube / Spotify URL.".to_string()
        };
        let empty = Paragraph::new(msg)
            .style(Style::default().fg(parse_color(&app.theme.colors.muted)))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(app.theme.border(focused))
                    .title(Span::styled(title, app.theme.accent())),
            );
        f.render_widget(empty, area);
    } else {
        f.render_stateful_widget(list, area, &mut app.library_state);
    }
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
                .title(Span::styled(title.clone(), app.theme.accent())),
        )
        .highlight_style(
            Style::default()
                .bg(parse_color(&app.theme.colors.secondary))
                .fg(parse_color(&app.theme.colors.background)),
        )
        .highlight_symbol(" ▶ ");

    app.layout.queue = inner_rect(area);
    if app.queue.is_empty() {
        let msg = " Queue is empty.\n\n Press Enter on a library item to play,\n or a to enqueue without playing.";
        let empty = Paragraph::new(msg)
            .style(Style::default().fg(parse_color(&app.theme.colors.muted)))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(app.theme.border(focused))
                    .title(Span::styled(title, app.theme.accent())),
            );
        f.render_widget(empty, area);
    } else {
        f.render_stateful_widget(list, area, &mut app.queue_state);
    }
}

fn inner_rect(r: Rect) -> Rect {
    Rect {
        x: r.x + 1,
        y: r.y + 1,
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    }
}

fn render_now_playing(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(false))
        .title(Span::styled(" Now Playing ", app.theme.accent()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(24)])
        .split(inner);
    let content_area = columns[0];
    let art_area = columns[1];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(content_area);

    if app.player.current().is_some() {
        if app.player.is_paused() {
            let art_text = app.theme.ascii.paused.clone();
            if !art_text.trim().is_empty() {
                let lines: Vec<Line> = art_text
                    .lines()
                    .map(|l| Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(parse_color(&app.theme.colors.primary)),
                    )))
                    .collect();
                f.render_widget(Paragraph::new(Text::from(lines)).alignment(Alignment::Center), art_area);
            }
        } else {
            render_spectrum_art(f, art_area, app);
        }
    }

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

    let queue_pos = match app.queue_index {
        Some(i) if !app.queue.is_empty() => {
            format!("  [{}/{}]", i + 1, app.queue.len())
        }
        _ => String::new(),
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
        Span::styled(
            queue_pos,
            Style::default().fg(parse_color(&app.theme.colors.muted)),
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
    app.layout.progress = chunks[1];
    app.layout.progress_total_ms = total.map(|d| d.as_millis() as u64).unwrap_or(0);

    let total_str = total
        .map(format_duration)
        .unwrap_or_else(|| "--:--".to_string());
    let hover_seek = app.hover_x.and_then(|hx| {
        let prog = app.layout.progress;
        if prog.width == 0 { return None; }
        let total_dur = total?;
        let frac = (hx.saturating_sub(prog.x)) as f32 / prog.width as f32;
        let secs = (total_dur.as_secs_f32() * frac.clamp(0.0, 1.0)) as u64;
        Some(format!("  → {:02}:{:02}", secs / 60, secs % 60))
    }).unwrap_or_default();
    let time = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} / {}", format_duration(elapsed), total_str),
            Style::default().fg(parse_color(&app.theme.colors.muted)),
        ),
        Span::styled(hover_seek, Style::default().fg(parse_color(&app.theme.colors.accent))),
    ]));
    f.render_widget(time, chunks[2]);

    let vol = build_volume_bar(app.player.volume(), 20, &app.theme);
    f.render_widget(Paragraph::new(vol), chunks[3]);

    let eq = app.player.eq().snapshot();
    let eq_line = build_eq_row(&eq, &app.theme);
    f.render_widget(Paragraph::new(eq_line), chunks[4]);

    let lyric_text = app
        .lyrics
        .as_ref()
        .and_then(|l| l.current_index(elapsed).map(|i| l.lines[i].text.clone()))
        .unwrap_or_default();
    if !lyric_text.is_empty() {
        let lyric = Paragraph::new(Line::from(Span::styled(
            format!(" ♪ {}", lyric_text),
            Style::default()
                .fg(parse_color(&app.theme.colors.accent))
                .add_modifier(Modifier::ITALIC),
        )));
        f.render_widget(lyric, chunks[5]);
    }
}

fn render_spectrum_art(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 3 || area.width < 8 {
        return;
    }

    let h = area.height as usize;
    let bar_rows = h.saturating_sub(2); // top row: notes, bottom row: label

    let primary = parse_color(&app.theme.colors.primary);
    let secondary = parse_color(&app.theme.colors.secondary);
    let accent = parse_color(&app.theme.colors.accent);
    let muted = parse_color(&app.theme.colors.muted);

    let bars = app.tap.compute_bars(6);
    let (_, _, treble) = app.tap.spectrum_bands();

    let elapsed = app.player.elapsed();
    let tick = (elapsed.as_millis() / 300) as usize;

    // Top row: animated note symbols driven by treble energy
    const NOTES: &[&str] = &["♪", "♫", "♬", "♩"];
    let note_count = (treble * 4.0).ceil() as usize + 1;
    let note_str: String = (0..note_count)
        .map(|i| NOTES[(tick + i) % NOTES.len()])
        .collect::<Vec<_>>()
        .join(" ");
    let note_line = Line::from(Span::styled(note_str, Style::default().fg(accent)));
    f.render_widget(
        Paragraph::new(note_line).alignment(Alignment::Center),
        Rect { x: area.x, y: area.y, width: area.width, height: 1 },
    );

    // Bar chart rows (rows 1 .. h-1)
    let n_bars = bars.len();
    let bar_w: usize = 2;
    let gap: usize = 1;
    let total_bar_w = n_bars * bar_w + (n_bars - 1) * gap;
    let pad: usize = (area.width as usize).saturating_sub(total_bar_w) / 2;

    let block_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let step = 1.0 / bar_rows as f32;

    for row in 0..bar_rows {
        let y = area.y + 1 + row as u16;
        let row_from_bottom = bar_rows - 1 - row;
        let threshold = row_from_bottom as f32 / bar_rows as f32;

        let mut spans: Vec<Span> = vec![Span::raw(" ".repeat(pad))];
        for (i, &val) in bars.iter().enumerate() {
            let ch: String = if val >= threshold + step {
                "█".repeat(bar_w)
            } else if val > threshold {
                let frac = (val - threshold) / step;
                let idx = ((frac * block_chars.len() as f32) as usize).min(block_chars.len() - 1);
                std::iter::repeat(block_chars[idx]).take(bar_w).collect()
            } else {
                " ".repeat(bar_w)
            };
            let color = if val > 0.75 { accent } else if val > 0.4 { primary } else { secondary };
            spans.push(Span::styled(ch, Style::default().fg(color)));
            if i + 1 < n_bars {
                spans.push(Span::raw(" "));
            }
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect { x: area.x, y, width: area.width, height: 1 },
        );
    }

    // Bottom row: label
    let label = Line::from(Span::styled("▶ NOW PLAY", Style::default().fg(muted)));
    f.render_widget(
        Paragraph::new(label).alignment(Alignment::Center),
        Rect { x: area.x, y: area.y + h as u16 - 1, width: area.width, height: 1 },
    );
}

fn build_eq_row(eq: &crate::eq::EqState, theme: &Theme) -> Line<'static> {
    let muted = parse_color(&theme.colors.muted);
    let accent = parse_color(&theme.colors.accent);
    let primary = parse_color(&theme.colors.primary);

    let band = |label: &str, db: f32| -> Vec<Span<'static>> {
        let color = if db == 0.0 { muted } else if db > 0.0 { accent } else { primary };
        vec![
            Span::styled(format!("{}: ", label), Style::default().fg(muted)),
            Span::styled(format!("{:+.0}dB", db), Style::default().fg(color)),
            Span::raw("  "),
        ]
    };

    let mut spans: Vec<Span<'static>> = vec![Span::styled(" ≡ EQ  ".to_string(), Style::default().fg(muted))];
    spans.extend(band("L", eq.low_db));
    spans.extend(band("M", eq.mid_db));
    spans.extend(band("H", eq.high_db));
    Line::from(spans)
}

fn build_volume_bar(volume: f32, width: usize, theme: &Theme) -> Line<'static> {
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

    let filled_color = Style::default().fg(parse_color(&theme.colors.progress_filled));
    let empty_color = Style::default().fg(parse_color(&theme.colors.progress_empty));
    let head_color = Style::default().fg(parse_color(&theme.colors.accent));

    let (fill, head, empt) = if filled == 0 {
        (String::new(), String::new(), theme.symbols.progress_empty.repeat(empty))
    } else if filled >= width {
        (theme.symbols.progress_fill.repeat(width), String::new(), String::new())
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

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner_char = SPINNER[(app.tick_count as usize / 3) % SPINNER.len()];

    let status_text = if app.url_editing {
        format!(" URL> {}_", app.url_input)
    } else if app.search_active() {
        format!(" /{}", app.search_query())
    } else if app.is_loading() {
        format!(" {} {}", spinner_char, app.status)
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
    let sleep = app
        .sleep_remaining()
        .map(|d| {
            let s = d.as_secs();
            format!("zzz {:02}:{:02} ", s / 60, s % 60)
        })
        .unwrap_or_default();
    let sort = format!("sort:{} ", app.sort.label());
    let hints = Paragraph::new(Span::styled(
        format!("{sleep}{shuf}{rep}{sort}[?] help [q] quit "),
        Style::default().fg(parse_color(&app.theme.colors.muted)),
    ))
    .alignment(Alignment::Right);
    f.render_widget(hints, chunks[1]);
}

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}
