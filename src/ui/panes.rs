use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::{
    album_art,
    app::{App, Pane},
    theme::parse_color,
    ui::util::{
        build_eq_row, build_progress, build_volume_bar, format_duration, highlight_match,
        inner_rect, scan_progress_suffix, status_style,
    },
};

pub fn render_mini(f: &mut Frame, area: Rect, app: &mut App) {
    let theme = &app.theme;
    let fg = parse_color(&theme.colors.foreground);
    let accent = parse_color(&theme.colors.accent);
    let muted = parse_color(&theme.colors.muted);

    let has_art = app.player.current().is_some()
        && (app.album_art.is_some() || !app.theme.ascii.paused.trim().is_empty());
    let (content_area, art_area) = if has_art && area.width >= 40 && area.height >= 4 {
        let art_w = (area.height * 2)
            .clamp(8, 16)
            .min(area.width.saturating_sub(25));
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(20), Constraint::Length(art_w)])
            .split(area);
        (cols[0], cols[1])
    } else {
        (area, Rect::default())
    };

    app.layout.art_area = art_area;

    if art_area.width > 0 && art_area.height > 0 && app.player.current().is_some() {
        if let Some(img) = &app.album_art {
            if app.art_picker.protocol == crate::album_art::Protocol::Blocks {
                album_art::render_blocks(f, art_area, img);
            }
        } else if app.player.is_paused() {
            let art_text = app.theme.ascii.paused.clone();
            if !art_text.trim().is_empty() {
                let lines: Vec<Line> = art_text
                    .lines()
                    .map(|l| {
                        Line::from(Span::styled(
                            l.to_string(),
                            Style::default().fg(parse_color(&app.theme.colors.primary)),
                        ))
                    })
                    .collect();
                f.render_widget(
                    Paragraph::new(Text::from(lines)).alignment(Alignment::Center),
                    art_area,
                );
            }
        }
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // track title + state
            Constraint::Length(1), // progress bar
            Constraint::Length(1), // time + volume + modes
            Constraint::Length(1), // eq row
            Constraint::Length(1), // status
        ])
        .split(content_area);

    // Row 0: play state + title
    let state_sym = if app.player.current().is_none() {
        theme.symbols.stop.clone()
    } else if app.player.is_paused() {
        theme.symbols.pause.clone()
    } else {
        theme.symbols.play.clone()
    };
    let title = app
        .player
        .current()
        .map(|t| t.display())
        .unwrap_or_else(|| "— nothing playing —".into());
    let queue_pos = match app.queue_index {
        Some(i) if !app.queue.is_empty() => format!(" [{}/{}]", i + 1, app.queue.len()),
        _ => String::new(),
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{state_sym}  "), Style::default().fg(accent)),
            Span::styled(title, Style::default().fg(fg).add_modifier(Modifier::BOLD)),
            Span::styled(queue_pos, Style::default().fg(muted)),
            Span::styled("  [m] full", Style::default().fg(muted)),
        ])),
        rows[0],
    );

    // Row 1: progress bar
    let elapsed = app.player.elapsed();
    let total = app.player.current().and_then(|t| t.duration);
    let progress_line = build_progress(
        elapsed,
        total,
        rows[1].width.saturating_sub(2) as usize,
        theme,
    );
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
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(
                " {} / {}  vol:{}%{shuf}{rep}{rg}",
                format_duration(elapsed),
                total_str,
                vol_pct
            ),
            Style::default().fg(muted),
        )])),
        rows[2],
    );

    // Row 3: EQ
    let eq = app.player.eq().snapshot();
    let eq_line = build_eq_row(&eq, theme);
    f.render_widget(Paragraph::new(eq_line), rows[3]);

    // Row 4: status
    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner_char = SPINNER[(app.tick_count as usize / 3) % SPINNER.len()];
    let status_text = if app.is_loading() {
        format!(
            " {} {}{}",
            spinner_char,
            app.status,
            scan_progress_suffix(app)
        )
    } else {
        format!(" {}", app.status)
    };
    f.render_widget(
        Paragraph::new(Span::styled(status_text, status_style(app))),
        rows[4],
    );
}

pub fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let logo_lines: Vec<Line> = theme
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

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(theme.border(false));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Logo centered
    f.render_widget(
        Paragraph::new(Text::from(logo_lines)).alignment(Alignment::Center),
        inner,
    );

    // System stats — bottom-right corner of the header inner area
    let s = &app.sys_stats;
    let net_str = if s.net_down_kbps >= 1024.0 {
        format!("↓ {:.1}MB/s", s.net_down_kbps / 1024.0)
    } else {
        format!("↓ {:.0}KB/s", s.net_down_kbps)
    };
    let stats_str = format!(" CPU {:.0}% · RAM {}MB · {} ", s.cpu_pct, s.ram_mb, net_str);
    let stats_width = stats_str.len() as u16;
    if inner.width > stats_width + 2 {
        let stats_area = Rect {
            x: inner.x + inner.width - stats_width,
            y: inner.y + inner.height.saturating_sub(1),
            width: stats_width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                stats_str,
                Style::default().fg(parse_color(&theme.colors.muted)),
            )),
            stats_area,
        );
    }
}

pub fn render_main(f: &mut Frame, area: Rect, app: &mut App) {
    if app.view_mode == crate::app::ViewMode::Radio {
        render_radio_view(f, area, app);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_library(f, chunks[0], app);
    render_queue(f, chunks[1], app);
}

pub fn render_library(f: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Pane::Library;
    let rows = app.library_rows();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            crate::app::LibraryRow::Track(t) => {
                let fav_span = if app.ratings.is_favorite(&t.path) {
                    Some(Span::styled(
                        "♥ ",
                        Style::default().fg(parse_color(&app.theme.colors.accent)),
                    ))
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
                if let Some(s) = fav_span {
                    spans.push(s);
                }
                let display = t.display();
                let fg = parse_color(&app.theme.colors.foreground);
                let needle = app.search_query();
                if !needle.is_empty() {
                    spans.extend(highlight_match(
                        &display,
                        needle,
                        fg,
                        parse_color(&app.theme.colors.accent),
                    ));
                } else {
                    spans.push(Span::styled(display, Style::default().fg(fg)));
                }
                spans.push(Span::styled(
                    dur,
                    Style::default().fg(parse_color(&app.theme.colors.muted)),
                ));
                ListItem::new(Line::from(spans))
            }
            crate::app::LibraryRow::Header(album) => ListItem::new(Line::from(Span::styled(
                format!("── {} ──", album),
                Style::default()
                    .fg(parse_color(&app.theme.colors.accent))
                    .add_modifier(Modifier::BOLD),
            ))),
            crate::app::LibraryRow::SmartHeader {
                label,
                count,
                expanded,
            } => {
                let icon = if *expanded { "▼" } else { "▶" };
                ListItem::new(Line::from(Span::styled(
                    format!(" {} {} ({})", icon, label, count),
                    Style::default()
                        .fg(parse_color(&app.theme.colors.accent))
                        .add_modifier(Modifier::BOLD),
                )))
            }
            crate::app::LibraryRow::Dir(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
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
            format!(
                " Recently Played [{}/{}] /{} ",
                shown,
                app.history.len(),
                app.search_query()
            )
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
        format!(
            " Library [{}/{}] /{} ",
            shown,
            app.library.len(),
            app.search_query()
        )
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
            format!(
                " No matches for /{}\n\n Press Esc to clear the search.",
                app.search_query()
            )
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

pub fn render_queue(f: &mut Frame, area: Rect, app: &mut App) {
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

    let endless_badge = if app.endless_mode {
        " [♾️ Auto-Play]"
    } else {
        ""
    };
    let title = if let Some(name) = &app.active_playlist_name {
        format!(" Queue ({}){} — {} ", app.queue.len(), endless_badge, name)
    } else {
        format!(" Queue ({}){} ", app.queue.len(), endless_badge)
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
                .bg(parse_color(&theme_color_secondary(&app.theme)))
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

fn theme_color_secondary(theme: &crate::theme::Theme) -> String {
    theme.colors.secondary.clone()
}

pub fn render_now_playing(f: &mut Frame, area: Rect, app: &mut App) {
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

    app.layout.art_area = art_area;

    if app.player.current().is_some() {
        if let Some(img) = &app.album_art {
            if app.art_picker.protocol == crate::album_art::Protocol::Blocks {
                album_art::render_blocks(f, art_area, img);
            }
        } else if app.player.is_paused() {
            let art_text = app.theme.ascii.paused.clone();
            if !art_text.trim().is_empty() {
                let lines: Vec<Line> = art_text
                    .lines()
                    .map(|l| {
                        Line::from(Span::styled(
                            l.to_string(),
                            Style::default().fg(parse_color(&app.theme.colors.primary)),
                        ))
                    })
                    .collect();
                f.render_widget(
                    Paragraph::new(Text::from(lines)).alignment(Alignment::Center),
                    art_area,
                );
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
    let hover_seek = app
        .hover_x
        .and_then(|hx| {
            let prog = app.layout.progress;
            if prog.width == 0 {
                return None;
            }
            let total_dur = total?;
            let frac = (hx.saturating_sub(prog.x)) as f32 / prog.width as f32;
            let secs = (total_dur.as_secs_f32() * frac.clamp(0.0, 1.0)) as u64;
            Some(format!("  → {:02}:{:02}", secs / 60, secs % 60))
        })
        .unwrap_or_default();
    let time = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} / {}", format_duration(elapsed), total_str),
            Style::default().fg(parse_color(&app.theme.colors.muted)),
        ),
        Span::styled(
            hover_seek,
            Style::default().fg(parse_color(&app.theme.colors.accent)),
        ),
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

pub fn render_spectrum_art(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 3 || area.width < 8 {
        return;
    }

    let h = area.height as usize;
    let bar_rows = h.saturating_sub(2);

    let accent = parse_color(&app.theme.colors.accent);

    let bars = app.tap.compute_bars(6);
    let (_, _, treble) = app.tap.spectrum_bands();

    let elapsed = app.player.elapsed();
    let tick = (elapsed.as_millis() / 300) as usize;

    const NOTES: &[&str] = &["♪", "♫", "♬", "♩"];
    let note_count = (treble * 4.0).ceil() as usize + 1;
    let note_str: String = (0..note_count)
        .map(|i| NOTES[(tick + i) % NOTES.len()])
        .collect::<Vec<_>>()
        .join(" ");
    let note_line = Line::from(Span::styled(note_str, Style::default().fg(accent)));
    f.render_widget(
        Paragraph::new(note_line).alignment(Alignment::Center),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );

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
                std::iter::repeat_n(block_chars[idx], bar_w).collect()
            } else {
                " ".repeat(bar_w)
            };
            let color = if val > 0.75 {
                parse_color(&app.theme.colors.accent)
            } else if val > 0.4 {
                parse_color(&app.theme.colors.primary)
            } else {
                parse_color(&app.theme.colors.secondary)
            };
            spans.push(Span::styled(ch, Style::default().fg(color)));
            if i + 1 < n_bars {
                spans.push(Span::raw(" ".repeat(gap)));
            }
        }
        let r = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }

    let vu_label = format!(
        " {:.0}% ",
        bars.iter().copied().fold(0.0f32, f32::max) * 100.0
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            vu_label,
            Style::default().fg(parse_color(&app.theme.colors.muted)),
        ))
        .alignment(Alignment::Center),
        Rect {
            x: area.x,
            y: area.y + h as u16 - 1,
            width: area.width,
            height: 1,
        },
    );
}

pub fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner_char = SPINNER[(app.tick_count as usize / 3) % SPINNER.len()];

    let status_text = if app.eq_preset_name_editing {
        format!(" EQ Preset name> {}_", app.eq_preset_name_input)
    } else if app.profile_name_editing {
        format!(" Profile name> {}_", app.profile_name_input)
    } else if app.playlist_name_editing {
        format!(" Name> {}_", app.playlist_name_input)
    } else if app.url_editing {
        format!(" URL> {}_", app.url_input)
    } else if app.search_active() {
        format!(" /{}", app.search_query())
    } else if app.is_loading() {
        format!(
            " {} {}{}",
            spinner_char,
            app.status,
            scan_progress_suffix(app)
        )
    } else {
        format!(" {}", app.status)
    };
    let status =
        Paragraph::new(Span::styled(status_text, status_style(app))).wrap(Wrap { trim: true });
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

pub fn render_radio_view(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);
    let accent = parse_color(&theme.colors.accent);
    let primary = parse_color(&theme.colors.primary);
    let secondary = parse_color(&theme.colors.secondary);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(26), // Col 1: Categories
            Constraint::Min(36),    // Col 2: Stations List
            Constraint::Length(32), // Col 3: Station Hub / Live Details
        ])
        .split(area);

    // --- Col 1: Categories ---
    let cat_focused = app.radio_focus_pane == 0;
    let cat_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(cat_focused))
        .title(Span::styled(
            " 📻 Categorias ",
            if cat_focused {
                theme.accent()
            } else {
                Style::default().fg(muted)
            },
        ));
    let cat_inner = cat_block.inner(chunks[0]);
    f.render_widget(cat_block, chunks[0]);

    let categories = crate::radio_browser::RadioCategory::ALL;
    let mut cat_items = Vec::new();
    for (i, cat) in categories.iter().enumerate() {
        let is_sel = i == app.radio_category_idx;
        let prefix = if is_sel { "▶ " } else { "  " };
        let style = if is_sel {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };
        cat_items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, Style::default().fg(accent)),
            Span::styled(cat.label(), style),
        ])));
    }
    f.render_widget(List::new(cat_items), cat_inner);

    // --- Col 2: Stations List & Search ---
    let stations_focused = app.radio_focus_pane == 1;
    let current_cat = categories
        .get(app.radio_category_idx)
        .copied()
        .unwrap_or(crate::radio_browser::RadioCategory::All);
    let stations_title = format!(" Estações ({}) ", current_cat.label());
    let stations_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(stations_focused))
        .title(Span::styled(
            stations_title,
            if stations_focused {
                theme.accent()
            } else {
                Style::default().fg(muted)
            },
        ));
    let stations_inner = stations_block.inner(chunks[1]);
    f.render_widget(stations_block, chunks[1]);

    let (list_rect, search_rect) = if current_cat == crate::radio_browser::RadioCategory::Search {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(2)])
            .split(stations_inner);
        (split[1], Some(split[0]))
    } else {
        (stations_inner, None)
    };

    if let Some(s_rect) = search_rect {
        let cursor = if app.radio_search_editing { "█" } else { "" };
        let s_text = if app.radio_search_query.is_empty() && !app.radio_search_editing {
            "  🔍 (pressione / para digitar a busca, Enter para buscar)".to_string()
        } else {
            format!("  🔍 {}{cursor}", app.radio_search_query)
        };
        let s_style = if app.radio_search_editing {
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted)
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(s_text, s_style)])),
            s_rect,
        );
    }

    let stations = app.radio_filtered_stations();
    if stations.is_empty() {
        let empty_msg = if current_cat == crate::radio_browser::RadioCategory::Favorites {
            "  Nenhuma rádio favoritada ainda.\n  Pressione 'f' em qualquer rádio para favoritar."
        } else if current_cat == crate::radio_browser::RadioCategory::Search {
            if app.radio_search_rx.is_some() {
                "  Buscando rádios online…"
            } else {
                "  Pressione '/' para buscar por nome, país ou gênero."
            }
        } else {
            "  Nenhuma estação encontrada nesta categoria."
        };
        f.render_widget(
            Paragraph::new(Span::styled(empty_msg, Style::default().fg(muted))),
            list_rect,
        );
    } else {
        let items_per_page = list_rect.height as usize;
        let scroll_offset = if app.radio_row >= items_per_page {
            app.radio_row - items_per_page + 1
        } else {
            0
        };

        let current_track_path = app
            .player
            .current()
            .map(|t| t.path.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut lines = Vec::new();
        for (i, &st) in stations
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(items_per_page)
        {
            let is_sel = i == app.radio_row;
            let is_playing = current_track_path == st.url;
            let is_fav = app.ratings.is_favorite(&std::path::PathBuf::from(&st.url));

            let prefix = if is_playing {
                "▶ "
            } else if is_sel {
                "→ "
            } else {
                "  "
            };

            let fav_icon = if is_fav { "♥ " } else { "  " };

            let name_style = if is_playing {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else if is_sel {
                Style::default().fg(fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };

            let country = st.country.as_deref().unwrap_or("World");
            let bitrate_str = st
                .bitrate
                .map(|b| format!("{b}k"))
                .unwrap_or_else(|| "128k".into());

            let row_line = Line::from(vec![
                Span::styled(prefix, Style::default().fg(accent)),
                Span::styled(fav_icon, Style::default().fg(accent)),
                Span::styled(format!("{:<26} ", st.name), name_style),
                Span::styled(format!(" {:<10} ", country), Style::default().fg(secondary)),
                Span::styled(
                    format!(" {:<5} ", bitrate_str),
                    Style::default().fg(primary),
                ),
            ]);
            lines.push(row_line);
        }
        f.render_widget(Paragraph::new(lines), list_rect);
    }

    // --- Col 3: Station Hub & Info ---
    let hub_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(false))
        .title(Span::styled(" Station Info ", theme.accent()));
    let hub_inner = hub_block.inner(chunks[2]);
    f.render_widget(hub_block, chunks[2]);

    let selected_station = stations.get(app.radio_row).copied();

    let mut info_lines = Vec::new();
    info_lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(muted)),
        Span::styled(
            if app.player.is_paused() {
                "⏸ Pausado"
            } else if app.is_loading() {
                "⏳ Conectando…"
            } else {
                "● LIVE STREAM"
            },
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
    ]));
    info_lines.push(Line::from(""));

    if let Some(st) = selected_station {
        info_lines.push(Line::from(vec![
            Span::styled("Estação:\n", Style::default().fg(muted)),
            Span::styled(
                format!(" {}\n", st.name),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ),
        ]));
        if let Some(c) = &st.country {
            info_lines.push(Line::from(vec![
                Span::styled("País: ", Style::default().fg(muted)),
                Span::styled(c.clone(), Style::default().fg(secondary)),
            ]));
        }
        if let Some(b) = st.bitrate {
            info_lines.push(Line::from(vec![
                Span::styled("Bitrate: ", Style::default().fg(muted)),
                Span::styled(format!("{b} kbps"), Style::default().fg(primary)),
            ]));
        }
        if !st.tags.is_empty() {
            info_lines.push(Line::from(vec![
                Span::styled("Gênero / Vibes:\n", Style::default().fg(muted)),
                Span::styled(format!(" {}\n", st.tags), Style::default().fg(muted)),
            ]));
        }
        if let Some(h) = &st.homepage {
            info_lines.push(Line::from(vec![
                Span::styled("Web: ", Style::default().fg(muted)),
                Span::styled(h.clone(), Style::default().fg(primary)),
            ]));
        }
    } else {
        info_lines.push(Line::from(Span::styled(
            "Nenhuma rádio selecionada",
            Style::default().fg(muted),
        )));
    }

    info_lines.push(Line::from(""));
    info_lines.push(Line::from(Span::styled(
        "Controles:",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    info_lines.push(Line::from(Span::styled(
        " Tab   alternar painel",
        Style::default().fg(fg),
    )));
    info_lines.push(Line::from(Span::styled(
        " Enter sintonizar rádio",
        Style::default().fg(fg),
    )));
    info_lines.push(Line::from(Span::styled(
        " a     enfileirar",
        Style::default().fg(fg),
    )));
    info_lines.push(Line::from(Span::styled(
        " +/N   adicionar rádio",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    info_lines.push(Line::from(Span::styled(
        " f     favoritar (♥)",
        Style::default().fg(fg),
    )));
    info_lines.push(Line::from(Span::styled(
        " /     buscar rádios",
        Style::default().fg(fg),
    )));
    info_lines.push(Line::from(Span::styled(
        " 1/2/4 outras visões",
        Style::default().fg(fg),
    )));

    f.render_widget(
        Paragraph::new(info_lines).wrap(Wrap { trim: true }),
        hub_inner,
    );
}
