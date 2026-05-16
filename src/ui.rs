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

    if app.mini_mode {
        render_mini(f, area, app);
        if app.show_help {
            render_help(f, area, &app.theme);
        }
        return;
    }

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
    if app.show_audio_panel {
        render_audio_panel(f, chunks[1], app);
    } else {
        render_main(f, chunks[1], app);
    }
    render_visualizer(f, chunks[2], app);
    render_now_playing(f, chunks[3], app);
    render_status(f, chunks[4], app);

    if app.show_help {
        render_help(f, area, &app.theme);
    }
    if app.show_info {
        render_track_info(f, area, app);
    }
    if app.show_playlist_browser {
        render_playlist_browser(f, area, app);
    }
    if app.show_device_selector {
        render_device_selector(f, area, app);
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

fn render_mini(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = &app.theme;
    let fg = parse_color(&theme.colors.foreground);
    let accent = parse_color(&theme.colors.accent);
    let muted = parse_color(&theme.colors.muted);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // track title + state
            Constraint::Length(1), // progress bar
            Constraint::Length(1), // time + volume + modes
            Constraint::Length(1), // eq row
            Constraint::Length(1), // status
        ])
        .split(area);

    // Row 0: play state + title
    let state_sym = if app.player.current().is_none() {
        theme.symbols.stop.clone()
    } else if app.player.is_paused() {
        theme.symbols.pause.clone()
    } else {
        theme.symbols.play.clone()
    };
    let title = app.player.current()
        .map(|t| t.display())
        .unwrap_or_else(|| "— nothing playing —".into());
    let queue_pos = match app.queue_index {
        Some(i) if !app.queue.is_empty() => format!(" [{}/{}]", i + 1, app.queue.len()),
        _ => String::new(),
    };
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(format!("{state_sym}  "), Style::default().fg(accent)),
        Span::styled(title, Style::default().fg(fg).add_modifier(Modifier::BOLD)),
        Span::styled(queue_pos, Style::default().fg(muted)),
        Span::styled("  [m] full", Style::default().fg(muted)),
    ])), rows[0]);

    // Row 1: progress bar
    let elapsed = app.player.elapsed();
    let total = app.player.current().and_then(|t| t.duration);
    let progress_line = build_progress(elapsed, total, rows[1].width.saturating_sub(2) as usize, theme);
    f.render_widget(Paragraph::new(progress_line), rows[1]);
    app.layout.progress = rows[1];
    app.layout.progress_total_ms = total.map(|d| d.as_millis() as u64).unwrap_or(0);

    // Row 2: time / volume / modes
    let total_str = total.map(format_duration).unwrap_or_else(|| "--:--".into());
    let vol_pct = (app.player.volume() * 100.0).round() as u32;
    let shuf = if app.shuffle { " shuf" } else { "" };
    let rep = match app.repeat {
        crate::app::RepeatMode::Off => "",
        crate::app::RepeatMode::All => " rep:all",
        crate::app::RepeatMode::One => " rep:one",
    };
    let rg = match app.replaygain_mode {
        crate::app::ReplayGainMode::Off => "",
        crate::app::ReplayGainMode::Track => " rg:trk",
        crate::app::ReplayGainMode::Album => " rg:alb",
    };
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} / {}  vol:{}%{shuf}{rep}{rg}", format_duration(elapsed), total_str, vol_pct),
            Style::default().fg(muted),
        ),
    ])), rows[2]);

    // Row 3: EQ
    let eq = app.player.eq().snapshot();
    let eq_line = build_eq_row(&eq, theme);
    f.render_widget(Paragraph::new(eq_line), rows[3]);

    // Row 4: status
    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner_char = SPINNER[(app.tick_count as usize / 3) % SPINNER.len()];
    let status_text = if app.is_loading() {
        format!(" {} {}", spinner_char, app.status)
    } else {
        format!(" {}", app.status)
    };
    f.render_widget(Paragraph::new(Span::styled(status_text, Style::default().fg(parse_color(&theme.colors.secondary)))), rows[4]);
}

fn render_playlist_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let entries = &app.playlist_browser_entries;

    let w = 60.min(area.width.saturating_sub(4));
    let h = (entries.len() as u16 + 5).clamp(6, area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let selected = i == app.playlist_browser_row;
            let deleting = app.playlist_browser_delete_confirm == Some(i);
            let style = if deleting {
                Style::default().fg(parse_color(&theme.colors.accent)).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default().fg(parse_color(&theme.colors.foreground)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(parse_color(&theme.colors.muted))
            };
            let label = format!(
                " {}{} ({} tracks){}",
                if selected { "▶ " } else { "  " },
                e.name,
                e.track_count,
                if deleting { "  ← confirm Shift+D" } else { "" },
            );
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let hint = Line::from(vec![
        Span::styled("  Enter", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(" load  ", Style::default().fg(parse_color(&theme.colors.muted))),
        Span::styled("a", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(" append  ", Style::default().fg(parse_color(&theme.colors.muted))),
        Span::styled("Shift+D", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(" delete  ", Style::default().fg(parse_color(&theme.colors.muted))),
        Span::styled("Esc", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(" close", Style::default().fg(parse_color(&theme.colors.muted))),
    ]);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(" Playlists ", theme.accent()))
                .title_bottom(hint),
        )
        .highlight_style(
            Style::default().bg(parse_color(&theme.colors.secondary)).fg(parse_color(&theme.colors.background)),
        );

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.playlist_browser_row));
    f.render_stateful_widget(list, popup, &mut state);
}

fn render_device_selector(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 70.min(area.width.saturating_sub(4));
    let h = (app.device_list.len() as u16 + 4).clamp(6, area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .device_list
        .iter()
        .map(|name| ListItem::new(Span::styled(
            format!("  {name}"),
            Style::default().fg(parse_color(&theme.colors.foreground)),
        )))
        .collect();

    let hint = Line::from(vec![
        Span::styled("  Enter", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(" select  ", Style::default().fg(parse_color(&theme.colors.muted))),
        Span::styled("Esc", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(" close", Style::default().fg(parse_color(&theme.colors.muted))),
    ]);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(" Output Device ", theme.accent()))
                .title_bottom(hint),
        )
        .highlight_style(
            Style::default()
                .bg(parse_color(&theme.colors.secondary))
                .fg(parse_color(&theme.colors.background)),
        );

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.device_selector_row));
    f.render_stateful_widget(list, popup, &mut state);
}

fn render_audio_panel(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let eq = app.player.eq().snapshot();
    let vol_pct = (app.player.volume() * 100.0).round() as i32;
    let xf = app.player.crossfade_secs;
    let sens = app.tap.sensitivity();
    let preset_name = crate::eq::PRESETS
        .get(app.eq_preset_idx)
        .map(|(n, _)| *n)
        .unwrap_or("?");

    struct Row {
        label: &'static str,
        value: String,
        bar_pct: f64,
    }

    let rows: &[Row] = &[
        Row { label: "EQ Low",       value: format!("{:+.0} dB", eq.low_db),  bar_pct: ((eq.low_db + 12.0) / 24.0) as f64 },
        Row { label: "EQ Mid",       value: format!("{:+.0} dB", eq.mid_db),  bar_pct: ((eq.mid_db + 12.0) / 24.0) as f64 },
        Row { label: "EQ High",      value: format!("{:+.0} dB", eq.high_db), bar_pct: ((eq.high_db + 12.0) / 24.0) as f64 },
        Row { label: "EQ Preset",    value: preset_name.to_string(),           bar_pct: app.eq_preset_idx as f64 / (crate::eq::PRESETS.len() - 1).max(1) as f64 },
        Row { label: "Volume",       value: format!("{}%", vol_pct),           bar_pct: (app.player.volume() / 1.5) as f64 },
        Row { label: "Crossfade",    value: format!("{:.1}s", xf),             bar_pct: (xf / 10.0) as f64 },
        Row { label: "Viz Sens",     value: format!("×{:.1}", sens),           bar_pct: ((sens - 0.1) / 2.9) as f64 },
    ];

    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);
    let accent = parse_color(&theme.colors.accent);
    let selected_bg = parse_color(&theme.colors.primary);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(" Audio Panel — ↑↓ select  ←→ adjust  e/Esc close ", theme.accent()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let bar_width = inner.width.saturating_sub(22) as usize;

    for (i, row) in rows.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let selected = i == app.audio_panel_row;
        let row_area = Rect { x: inner.x, y, width: inner.width, height: 1 };

        let label_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };
        let prefix = if selected { "▶ " } else { "  " };

        let filled = (row.bar_pct.clamp(0.0, 1.0) * bar_width as f64).round() as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

        let line = Line::from(vec![
            Span::styled(format!("{}{:<12}", prefix, row.label), label_style),
            Span::styled(format!(" {:>8}  ", row.value), Style::default().fg(muted)),
            Span::styled(bar, if selected {
                Style::default().fg(accent)
            } else {
                Style::default().fg(parse_color(&theme.colors.muted))
            }),
        ]);

        let row_style = if selected {
            Style::default().bg(selected_bg)
        } else {
            Style::default()
        };
        f.render_widget(Paragraph::new(line).style(row_style), row_area);
    }

    let hint_y = inner.y + App::AUDIO_PANEL_ROWS as u16 + 1;
    if hint_y < inner.y + inner.height {
        let hint_area = Rect { x: inner.x, y: hint_y, width: inner.width, height: 1 };
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("  ← / → to adjust  |  ↑↓ to navigate  |  e or Esc to close", Style::default().fg(muted)),
        ]));
        f.render_widget(hint, hint_area);
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
        Line::from("    v          cycle mode (spectrum/waveform/vu-meter)"),
        Line::from("    [ / ]      sensitivity -/+ (×0.1..×3.0)"),
        Line::from("    Shift+G    cycle ReplayGain (off/track/album)"),
        Line::from(""),
        Line::from("  Audio panel"),
        Line::from("    e          open/close panel"),
        Line::from("    ↑↓ / jk    select parameter"),
        Line::from("    ← / →      decrease / increase"),
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
    let mode_label = app.viz_mode.label();
    let title = format!(" {} (×{:.1}) [v] ", mode_label, app.tap.sensitivity());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(false))
        .title(Span::styled(title, app.theme.accent()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    match app.viz_mode {
        crate::app::VizMode::Spectrum => render_viz_spectrum(f, inner, app),
        crate::app::VizMode::Waveform => render_viz_waveform(f, inner, app),
        crate::app::VizMode::VuMeter => render_viz_vu(f, inner, app),
    }
}

fn render_viz_spectrum(f: &mut Frame, inner: Rect, app: &App) {
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
            let color = if val > 0.75 { accent } else if val > 0.4 { primary } else { secondary };
            let bar_text: String = std::iter::repeat(ch).take(bar_w as usize).collect();
            spans.push(Span::styled(bar_text, Style::default().fg(color)));
            if i + 1 < n_bars {
                spans.push(Span::raw(" ".repeat(gap as usize)));
            }
        }
        let r = Rect { x: inner.x, y: inner.y + row as u16, width: inner.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }
}

fn render_viz_waveform(f: &mut Frame, inner: Rect, app: &App) {
    let w = inner.width as usize;
    let h = inner.height as usize;
    let samples = app.tap.raw_snapshot(w);
    let mid = h / 2;
    let accent = parse_color(&app.theme.colors.accent);
    let primary = parse_color(&app.theme.colors.primary);
    let muted = parse_color(&app.theme.colors.muted);

    for row in 0..h {
        let mut spans: Vec<Span> = Vec::with_capacity(w);
        for (col, &s) in samples.iter().enumerate() {
            let sample_row = mid as i32 - (s * mid as f32) as i32;
            let ch = if row as i32 == sample_row {
                '█'
            } else if row as i32 == mid as i32 && col % 8 == 0 {
                '·'
            } else {
                ' '
            };
            let color = if ch == '█' {
                if s.abs() > 0.7 { accent } else { primary }
            } else {
                muted
            };
            spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
        }
        let r = Rect { x: inner.x, y: inner.y + row as u16, width: inner.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }
}

fn render_viz_vu(f: &mut Frame, inner: Rect, app: &App) {
    let rms = app.tap.rms_level();
    let peak_bars = app.tap.compute_bars(1);
    let peak = peak_bars.first().copied().unwrap_or(0.0);

    let w = inner.width as usize;
    let accent = parse_color(&app.theme.colors.accent);
    let primary = parse_color(&app.theme.colors.primary);
    let secondary = parse_color(&app.theme.colors.secondary);
    let muted = parse_color(&app.theme.colors.muted);

    let bar_row = |f: &mut Frame, y: u16, label: &str, level: f32, color_hi: ratatui::style::Color| {
        let label_w = 5usize;
        let bar_w = w.saturating_sub(label_w + 2);
        let filled = (level.clamp(0.0, 1.0) * bar_w as f32).round() as usize;
        let empty = bar_w - filled;
        let spans = vec![
            Span::styled(format!("{:>5} ", label), Style::default().fg(muted)),
            Span::styled("█".repeat(filled), Style::default().fg(color_hi)),
            Span::styled("░".repeat(empty), Style::default().fg(secondary)),
            Span::styled(format!(" {:3.0}%", level * 100.0), Style::default().fg(muted)),
        ];
        let r = Rect { x: inner.x, y, width: inner.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    };

    let mid = inner.y + inner.height / 2;
    bar_row(f, mid.saturating_sub(1), "RMS", rms, if rms > 0.8 { accent } else { primary });
    bar_row(f, mid, "Peak", peak, if peak > 0.75 { accent } else { primary });
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
                let fav_span = if app.ratings.is_favorite(&t.path) {
                    Some(Span::styled("♥ ", Style::default().fg(parse_color(&app.theme.colors.accent))))
                } else {
                    None
                };
                let dur = t
                    .duration
                    .map(|d| {
                        let s = d.as_secs();
                        format!("  {:02}:{:02}", s / 60, s % 60)
                    })
                    .unwrap_or_default();
                let mut spans = Vec::new();
                if let Some(s) = fav_span { spans.push(s); }
                spans.push(Span::styled(
                    t.display(),
                    Style::default().fg(parse_color(&app.theme.colors.foreground)),
                ));
                spans.push(Span::styled(dur, Style::default().fg(parse_color(&app.theme.colors.muted))));
                ListItem::new(Line::from(spans))
            }
            crate::app::LibraryRow::Header(album) => ListItem::new(Line::from(Span::styled(
                format!("── {} ──", album),
                Style::default()
                    .fg(parse_color(&app.theme.colors.accent))
                    .add_modifier(Modifier::BOLD),
            ))),
            crate::app::LibraryRow::SmartHeader { label, count, expanded } => {
                let icon = if *expanded { "▼" } else { "▶" };
                ListItem::new(Line::from(Span::styled(
                    format!(" {} {} ({})", icon, label, count),
                    Style::default()
                        .fg(parse_color(&app.theme.colors.accent))
                        .add_modifier(Modifier::BOLD),
                )))
            }
            crate::app::LibraryRow::Dir(p) => {
                let name = p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                ListItem::new(Line::from(Span::styled(
                    format!("📁 {}/", name),
                    Style::default().fg(parse_color(&app.theme.colors.accent)),
                )))
            }
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
    } else if app.view_mode == crate::app::ViewMode::Smart {
        " Smart Playlists ".to_string()
    } else if app.view_mode == crate::app::ViewMode::Browser {
        let path = app.browser_current_path();
        let display = path.to_string_lossy();
        format!(" 📁 {} ", display)
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
        } else if app.view_mode == crate::app::ViewMode::Browser {
            " Empty directory.".to_string()
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

    let title = if let Some(name) = &app.active_playlist_name {
        format!(" Queue ({}) — {} ", app.queue.len(), name)
    } else {
        format!(" Queue ({}) ", app.queue.len())
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

    let status_text = if app.playlist_name_editing {
        format!(" Name> {}_", app.playlist_name_input)
    } else if app.url_editing {
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
    let rg = match app.replaygain_mode {
        crate::app::ReplayGainMode::Off => String::new(),
        crate::app::ReplayGainMode::Track => "rg:track ".to_string(),
        crate::app::ReplayGainMode::Album => "rg:album ".to_string(),
    };
    let hints = Paragraph::new(Span::styled(
        format!("{sleep}{shuf}{rep}{rg}{sort}[?] help [q] quit "),
        Style::default().fg(parse_color(&app.theme.colors.muted)),
    ))
    .alignment(Alignment::Right);
    f.render_widget(hints, chunks[1]);
}

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}
