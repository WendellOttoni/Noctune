mod help;
mod visualizer;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;

use crate::{
    album_art,
    app::{App, Pane},
    theme::{parse_color, Theme},
};

use self::help::render_help;
use self::visualizer::render_visualizer;

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    if app.mini_mode {
        render_mini(f, area, app);
        if app.show_help {
            render_help(f, area, app.help_scroll, &app.theme);
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

    render_header(f, chunks[0], app);
    if app.show_audio_panel {
        render_audio_panel(f, chunks[1], app);
    } else {
        render_main(f, chunks[1], app);
    }
    render_visualizer(f, chunks[2], app);
    render_now_playing(f, chunks[3], app);
    render_status(f, chunks[4], app);

    if app.show_help {
        render_help(f, area, app.help_scroll, &app.theme);
    }
    if app.show_stats {
        render_stats(f, area, app);
    }
    if app.show_lastfm_panel {
        render_lastfm(f, area, app);
    }
    if app.show_info {
        render_track_info(f, area, app);
    }
    if app.show_playlist_browser {
        render_playlist_browser(f, area, app);
    }
    if app.show_profile_browser {
        render_profile_browser(f, area, app);
    }
    if app.show_spotify_browser {
        render_spotify_browser(f, area, app);
    }
    if app.show_device_selector {
        render_device_selector(f, area, app);
    }
    if app.show_eq_tuner {
        render_eq_tuner(f, area, app);
    }
    if app.show_tag_editor {
        render_tag_editor(f, area, app);
    }
    if app.show_radio_browser {
        render_radio_browser(f, area, app);
    }
    if app.show_radio_custom_modal {
        render_radio_custom_modal(f, area, app);
    }
    if app.show_lyrics {
        render_lyrics_modal(f, area, app);
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
            Span::styled(
                value,
                Style::default().fg(parse_color(&app.theme.colors.foreground)),
            ),
        ])
    };
    let dash = "—".to_string();

    let dur_str = meta
        .duration
        .map(|d| {
            let s = d.as_secs();
            format!("{:02}:{:02}", s / 60, s % 60)
        })
        .unwrap_or_else(|| dash.clone());

    let rate_str = meta
        .sample_rate
        .map(|r| format!("{} Hz", r))
        .unwrap_or_else(|| dash.clone());
    let chans_str = meta
        .channels
        .map(|c| format!("{}", c))
        .unwrap_or_else(|| dash.clone());
    let bits_str = meta
        .bits_per_sample
        .map(|b| format!("{} bit", b))
        .unwrap_or_else(|| dash.clone());

    let lines: Vec<Line> = vec![
        row("Title", meta.title.unwrap_or_else(|| track.title.clone())),
        row("Artist", meta.artist.unwrap_or_else(|| dash.clone())),
        row(
            "Album Artist",
            meta.album_artist.unwrap_or_else(|| dash.clone()),
        ),
        row("Album", meta.album.unwrap_or_else(|| dash.clone())),
        row("Year", meta.year.unwrap_or_else(|| dash.clone())),
        row("Genre", meta.genre.unwrap_or_else(|| dash.clone())),
        row("Track #", meta.track_number.unwrap_or_else(|| dash.clone())),
        Line::from(""),
        row("Duration", dur_str),
        row("Codec", meta.codec.unwrap_or_else(|| dash.clone())),
        row("Sample rate", rate_str),
        row("Channels", chans_str),
        row("Bit depth", bits_str),
        Line::from(""),
        row("Path", track.path.display().to_string()),
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
            .title(Span::styled(
                " Track info — press any key ",
                app.theme.accent(),
            )),
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

/// `" [done/total]"` if a scan is reporting progress, else empty (#104).
fn scan_progress_suffix(app: &App) -> String {
    match app.scan_progress {
        Some((d, t)) if t > 0 => format!(" [{d}/{t}]"),
        _ => String::new(),
    }
}

/// Pick the status-bar foreground color from the current `StatusKind` (#102).
/// Error → red, Warning → yellow, Info → theme.secondary.
fn status_style(app: &App) -> Style {
    use crate::app::StatusKind;
    use ratatui::style::{Color, Modifier};
    match app.status_kind {
        StatusKind::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        StatusKind::Warning => Style::default().fg(Color::Yellow),
        StatusKind::Info => Style::default().fg(parse_color(&app.theme.colors.secondary)),
    }
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
                Style::default()
                    .fg(parse_color(&theme.colors.accent))
                    .add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default()
                    .fg(parse_color(&theme.colors.foreground))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(parse_color(&theme.colors.muted))
            };
            // #84: surface the last time this playlist was played, if known.
            let recency = app
                .play_history
                .playlist_record(&crate::history::PlaylistRef::Local {
                    path: e.path.clone(),
                })
                .map(|r| format!(" · played {}", relative_time(r.last_played)))
                .unwrap_or_default();
            let label = format!(
                " {}{} ({} tracks){}{}",
                if selected { "▶ " } else { "  " },
                e.name,
                e.track_count,
                recency,
                if deleting {
                    "  ← confirm Shift+D"
                } else {
                    ""
                },
            );
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let hint = Line::from(vec![
        Span::styled(
            "  Enter",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " load  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled("a", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(
            " append  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled(
            "Shift+D",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " delete  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled(
            "Esc",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " close",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
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
            Style::default()
                .bg(parse_color(&theme.colors.secondary))
                .fg(parse_color(&theme.colors.background)),
        );

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.playlist_browser_row));
    f.render_stateful_widget(list, popup, &mut state);
}

fn render_profile_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let profiles = &app.profiles;

    let w = 60.min(area.width.saturating_sub(4));
    let h = (profiles.len() as u16 + 6).clamp(7, area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = if profiles.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No profiles yet. Press n to save current settings.",
            Style::default().fg(parse_color(&theme.colors.muted)),
        )))]
    } else {
        profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let selected = i == app.profile_browser_row;
                let style = if selected {
                    Style::default()
                        .fg(parse_color(&theme.colors.foreground))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(parse_color(&theme.colors.muted))
                };
                let label = format!(
                    " {}{}  vol:{:.0}%  EQ:{:+.0}/{:+.0}/{:+.0}  theme:{}",
                    if selected { "▶ " } else { "  " },
                    p.name,
                    p.volume * 100.0,
                    p.eq_low_db,
                    p.eq_mid_db,
                    p.eq_high_db,
                    p.theme,
                );
                ListItem::new(Line::from(Span::styled(label, style)))
            })
            .collect()
    };

    let hint = Line::from(vec![
        Span::styled(
            "  Enter",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " load  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled("n", Style::default().fg(parse_color(&theme.colors.accent))),
        Span::styled(
            " save current  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled(
            "Shift+D",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " delete  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled(
            "Esc",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " close",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
    ]);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(" Profiles ", theme.accent()))
                .title_bottom(hint),
        )
        .highlight_style(
            Style::default()
                .bg(parse_color(&theme.colors.secondary))
                .fg(parse_color(&theme.colors.background)),
        );

    let mut state = ratatui::widgets::ListState::default();
    state.select(if profiles.is_empty() {
        None
    } else {
        Some(app.profile_browser_row)
    });
    f.render_stateful_widget(list, popup, &mut state);
}

fn render_spotify_browser(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::SpotifyTab;
    let theme = &app.theme;
    let accent = parse_color(&theme.colors.accent);
    let muted = parse_color(&theme.colors.muted);
    let fg = parse_color(&theme.colors.foreground);
    let bg = parse_color(&theme.colors.background);
    let secondary = parse_color(&theme.colors.secondary);

    let w = 80.min(area.width.saturating_sub(2));
    let h = area.height.saturating_sub(4).max(10);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    // Tab labels
    let tab_line = Line::from(vec![
        Span::styled(
            if app.spotify_browser_tab == SpotifyTab::Search {
                " [Search] "
            } else {
                "  Search  "
            },
            if app.spotify_browser_tab == SpotifyTab::Search {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
        Span::styled(
            if app.spotify_browser_tab == SpotifyTab::MyPlaylists {
                " [My Playlists] "
            } else {
                "  My Playlists  "
            },
            if app.spotify_browser_tab == SpotifyTab::MyPlaylists {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
        Span::styled(
            if app.spotify_browser_tab == SpotifyTab::LikedSongs {
                " [Liked Songs] "
            } else {
                "  Liked Songs  "
            },
            if app.spotify_browser_tab == SpotifyTab::LikedSongs {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
    ]);

    let hint = Line::from(vec![
        Span::styled("  Tab", Style::default().fg(accent)),
        Span::styled(" switch  ", Style::default().fg(muted)),
        Span::styled("/", Style::default().fg(accent)),
        Span::styled(" search  ", Style::default().fg(muted)),
        Span::styled("Enter", Style::default().fg(accent)),
        Span::styled(" play  ", Style::default().fg(muted)),
        Span::styled("a", Style::default().fg(accent)),
        Span::styled(" enqueue  ", Style::default().fg(muted)),
        Span::styled("Esc", Style::default().fg(accent)),
        Span::styled(" close", Style::default().fg(muted)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " Spotify ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(hint);
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    // Layout: search bar (2 rows) + tab line (1) + results list (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    // Search bar
    let query_display = if app.spotify_browser_query_editing {
        format!(" Search: {}█", app.spotify_browser_query)
    } else if app.spotify_browser_query.is_empty() {
        " Press / to search tracks…".to_string()
    } else {
        format!(" Search: {}", app.spotify_browser_query)
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            query_display,
            Style::default().fg(if app.spotify_browser_query_editing {
                fg
            } else {
                muted
            }),
        )),
        chunks[0],
    );

    // Tab bar
    f.render_widget(Paragraph::new(tab_line), chunks[1]);

    // Results
    let list_area = chunks[2];
    match app.spotify_browser_tab {
        SpotifyTab::Search | SpotifyTab::LikedSongs => {
            let results = &app.spotify_browser_results;
            if results.is_empty() {
                let msg = if app.spotify_browser_tab == SpotifyTab::LikedSongs {
                    "  Loading liked songs…"
                } else {
                    "  No results. Type a query and press Enter."
                };
                f.render_widget(
                    Paragraph::new(Span::styled(msg, Style::default().fg(muted))),
                    list_area,
                );
            } else {
                let items: Vec<ListItem> = results
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let selected = i == app.spotify_browser_row;
                        let dur = t
                            .duration
                            .map(|d| {
                                let s = d.as_secs();
                                format!("{:02}:{:02}", s / 60, s % 60)
                            })
                            .unwrap_or_default();
                        let label = format!(
                            " {} {:<40} {:<24} {}",
                            if selected { "▶" } else { " " },
                            t.title.chars().take(38).collect::<String>(),
                            t.artist
                                .as_deref()
                                .unwrap_or("")
                                .chars()
                                .take(22)
                                .collect::<String>(),
                            dur,
                        );
                        let style = if selected {
                            Style::default().fg(fg).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(muted)
                        };
                        ListItem::new(Line::from(Span::styled(label, style)))
                    })
                    .collect();
                let mut state = ratatui::widgets::ListState::default();
                state.select(Some(app.spotify_browser_row));
                f.render_stateful_widget(
                    List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
                    list_area,
                    &mut state,
                );
            }
        }
        SpotifyTab::MyPlaylists => {
            if app.spotify_my_playlists.is_empty() {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "  Loading playlists…",
                        Style::default().fg(muted),
                    )),
                    list_area,
                );
            } else {
                let items: Vec<ListItem> = app
                    .spotify_my_playlists
                    .iter()
                    .enumerate()
                    .map(|(i, (_, name, count))| {
                        let selected = i == app.spotify_playlist_row;
                        let label = format!(
                            " {} {:<50} {} tracks",
                            if selected { "▶" } else { " " },
                            name.chars().take(48).collect::<String>(),
                            count,
                        );
                        let style = if selected {
                            Style::default().fg(fg).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(muted)
                        };
                        ListItem::new(Line::from(Span::styled(label, style)))
                    })
                    .collect();
                let mut state = ratatui::widgets::ListState::default();
                state.select(Some(app.spotify_playlist_row));
                f.render_stateful_widget(
                    List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
                    list_area,
                    &mut state,
                );
            }
        }
    }
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
        .map(|name| {
            ListItem::new(Span::styled(
                format!("  {name}"),
                Style::default().fg(parse_color(&theme.colors.foreground)),
            ))
        })
        .collect();

    let hint = Line::from(vec![
        Span::styled(
            "  Enter",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " select  ",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
        Span::styled(
            "Esc",
            Style::default().fg(parse_color(&theme.colors.accent)),
        ),
        Span::styled(
            " close",
            Style::default().fg(parse_color(&theme.colors.muted)),
        ),
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

fn render_eq_tuner(f: &mut Frame, area: Rect, app: &App) {
    use std::collections::HashSet;

    let theme = &app.theme;
    let eq = app.player.eq().snapshot();

    const CURVE_H: usize = 13; // rows for curve (+12 to -12 dB, 2dB per row)
    let w = 72.min(area.width.saturating_sub(4)).max(44);
    // 1 band header + curve + 1 freq row + 1 blank + 3 sliders + 1 blank + 1 preset + 1 hint + 2 border
    let h = (CURVE_H as u16 + 11).min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);
    let accent = parse_color(&theme.colors.accent);
    let secondary = parse_color(&theme.colors.secondary);
    let bg = parse_color(&theme.colors.background);

    let inner_w = (w as usize).saturating_sub(2);
    let curve_w = inner_w.saturating_sub(6).max(20);

    let freq_to_col = |freq: f32| -> usize {
        let t = (freq / 20.0).log10() / (20000.0f32 / 20.0).log10();
        ((t * (curve_w - 1) as f32).round() as usize).min(curve_w - 1)
    };

    let low_col = freq_to_col(200.0);
    let mid_col = freq_to_col(1000.0);
    let high_col = freq_to_col(5000.0);

    // Subtle vertical grid lines at non-band key frequencies
    let grid_cols: HashSet<usize> = [50.0f32, 100.0, 500.0, 2000.0, 10000.0]
        .iter()
        .map(|&freq| freq_to_col(freq))
        .collect();

    // Frequency response curve (approximated transfer functions for display)
    let curve_rows: Vec<usize> = (0..curve_w)
        .map(|col| {
            let t = col as f32 / (curve_w - 1).max(1) as f32;
            let freq = 20.0f32 * (1000.0f32).powf(t);
            let fc_low = 200.0f32;
            let fc_mid = 1000.0f32;
            let fc_high = 5000.0f32;
            let g_low = eq.low_db * fc_low * fc_low / (freq * freq + fc_low * fc_low);
            let bw = freq / fc_mid - fc_mid / freq;
            let g_mid = eq.mid_db / (1.0 + (bw * 0.9).powi(2));
            let g_high = eq.high_db * freq * freq / (freq * freq + fc_high * fc_high);
            let total = (g_low + g_mid + g_high).clamp(-12.0, 12.0);
            let row = ((12.0 - total) / 24.0 * (CURVE_H - 1) as f32).round() as usize;
            row.min(CURVE_H - 1)
        })
        .collect();

    let zero_row = (CURVE_H - 1) / 2; // row 6 = 0 dB

    let mut lines: Vec<Line> = Vec::new();

    // Band selector header: numbered nodes show which band is active
    {
        let nodes = [
            (low_col, "1", app.eq_tuner_band == 0),
            (mid_col, "2", app.eq_tuner_band == 1),
            (high_col, "3", app.eq_tuner_band == 2),
        ];
        let mut spans = vec![Span::styled("      │", Style::default().fg(muted))];
        let mut cursor = 0usize;
        for &(col, sym, selected) in &nodes {
            if col > cursor {
                spans.push(Span::raw(" ".repeat(col - cursor)));
                cursor = col;
            }
            let style = if selected {
                Style::default()
                    .fg(bg)
                    .bg(accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            };
            spans.push(Span::styled(sym, style));
            cursor += 1;
        }
        if cursor < curve_w {
            spans.push(Span::raw(" ".repeat(curve_w - cursor)));
        }
        lines.push(Line::from(spans));
    }

    // Curve rows
    for row in 0..CURVE_H {
        let db = 12.0 - row as f32 * 24.0 / (CURVE_H - 1) as f32;
        let label = if (db.round() as i32).abs() % 6 == 0 {
            format!("{:+3.0} ", db)
        } else {
            "     ".to_string()
        };
        let axis_char = if row == zero_row { "┼" } else { "│" };

        let mut spans: Vec<Span> = vec![
            Span::styled(label, Style::default().fg(muted)),
            Span::styled(axis_char, Style::default().fg(muted)),
        ];

        for col in 0..curve_w {
            let cr = curve_rows[col] as isize;
            let r = row as isize;
            let zr = zero_row as isize;

            let is_on_curve = cr == r;

            // Vertical connector for steep diagonal transitions between adjacent columns
            let is_vert = if col > 0 && col < curve_w - 1 {
                let prev = curve_rows[col - 1] as isize;
                let next = curve_rows[col + 1] as isize;
                (prev < r && r < next) || (next < r && r < prev)
            } else {
                false
            };

            // Fill region between curve and zero line
            let in_boost = cr < zr && r > cr && r < zr;
            let in_cut = cr > zr && r > zr && r < cr;

            // Numbered nodes at band frequency columns, positioned on the curve
            let is_low_node = col == low_col && is_on_curve;
            let is_mid_node = col == mid_col && is_on_curve;
            let is_high_node = col == high_col && is_on_curve;

            let is_grid_col = grid_cols.contains(&col);

            let span = if is_low_node {
                let style = if app.eq_tuner_band == 0 {
                    Style::default()
                        .fg(bg)
                        .bg(accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(secondary)
                };
                Span::styled("1", style)
            } else if is_mid_node {
                let style = if app.eq_tuner_band == 1 {
                    Style::default()
                        .fg(bg)
                        .bg(accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(secondary)
                };
                Span::styled("2", style)
            } else if is_high_node {
                let style = if app.eq_tuner_band == 2 {
                    Style::default()
                        .fg(bg)
                        .bg(accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(secondary)
                };
                Span::styled("3", style)
            } else if is_on_curve {
                Span::styled("─", Style::default().fg(accent))
            } else if is_vert {
                Span::styled("│", Style::default().fg(accent))
            } else if in_boost {
                Span::styled("░", Style::default().fg(accent))
            } else if in_cut {
                Span::styled("░", Style::default().fg(secondary))
            } else if r == zr {
                if is_grid_col {
                    Span::styled("┼", Style::default().fg(muted))
                } else {
                    Span::styled("─", Style::default().fg(muted))
                }
            } else if is_grid_col {
                Span::styled("╎", Style::default().fg(muted))
            } else {
                Span::raw(" ")
            };
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }

    // Frequency axis labels — denser than before
    {
        let freq_markers: &[(&str, f32)] = &[
            ("20", 20.0),
            ("50", 50.0),
            ("100", 100.0),
            ("200", 200.0),
            ("500", 500.0),
            ("1k", 1000.0),
            ("2k", 2000.0),
            ("5k", 5000.0),
            ("10k", 10000.0),
            ("20k", 20000.0),
        ];
        let mut freq_row = vec![' '; curve_w];
        for (label, freq) in freq_markers {
            let t = (freq / 20.0).log10() / (20000.0f32 / 20.0).log10();
            let col = (t * (curve_w - 1) as f32).round() as usize;
            let col = col.min(curve_w.saturating_sub(label.len()));
            for (i, ch) in label.chars().enumerate() {
                if col + i < curve_w && freq_row[col + i] == ' ' {
                    freq_row[col + i] = ch;
                }
            }
        }
        let freq_str: String = freq_row.iter().collect();
        lines.push(Line::from(vec![
            Span::styled("     ", Style::default()),
            Span::styled("└", Style::default().fg(muted)),
            Span::styled(freq_str, Style::default().fg(muted)),
        ]));
    }

    lines.push(Line::from(""));

    // Band sliders
    let bands = [
        ("Low  200Hz", eq.low_db, app.eq_tuner_band == 0),
        ("Mid  1kHz ", eq.mid_db, app.eq_tuner_band == 1),
        ("High 5kHz ", eq.high_db, app.eq_tuner_band == 2),
    ];
    let slider_w = inner_w.saturating_sub(26).max(10);
    for (label, db, selected) in &bands {
        let cursor = if *selected { "▶" } else { " " };
        let cursor_style = if *selected {
            Style::default().fg(accent)
        } else {
            Style::default().fg(muted)
        };
        let pos = ((db + 12.0) / 24.0 * (slider_w - 1) as f32).round() as usize;
        let pos = pos.min(slider_w - 1);
        let bar: String = (0..slider_w)
            .map(|i| if i == pos { '●' } else { '─' })
            .collect();
        let db_label = format!("{:+.0} dB", db);
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", cursor), cursor_style),
            Span::styled(
                *label,
                Style::default().fg(if *selected { fg } else { muted }),
            ),
            Span::styled("  ├", Style::default().fg(muted)),
            Span::styled(
                bar,
                Style::default().fg(if *selected { accent } else { secondary }),
            ),
            Span::styled("┤", Style::default().fg(muted)),
            Span::styled(
                format!(" {:<7}", db_label),
                Style::default().fg(if *selected { accent } else { fg }),
            ),
        ]));
    }

    lines.push(Line::from(""));

    // Preset buttons
    let current = crate::eq::PRESETS.iter().position(|(_, s)| {
        (s.low_db - eq.low_db).abs() < 0.1
            && (s.mid_db - eq.mid_db).abs() < 0.1
            && (s.high_db - eq.high_db).abs() < 0.1
    });
    let mut preset_spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (i, (name, _)) in crate::eq::PRESETS.iter().enumerate() {
        let active = current == Some(i);
        let style = if active {
            Style::default().fg(bg).bg(accent)
        } else {
            Style::default().fg(muted)
        };
        preset_spans.push(Span::styled(format!(" {} ", name), style));
        preset_spans.push(Span::raw(" "));
    }
    lines.push(Line::from(preset_spans));

    lines.push(Line::from(vec![
        Span::styled("  ← → ", Style::default().fg(accent)),
        Span::styled("adjust  ", Style::default().fg(muted)),
        Span::styled("↑↓ ", Style::default().fg(accent)),
        Span::styled("band  ", Style::default().fg(muted)),
        Span::styled("0 ", Style::default().fg(accent)),
        Span::styled("preset  ", Style::default().fg(muted)),
        Span::styled("Esc ", Style::default().fg(accent)),
        Span::styled("close", Style::default().fg(muted)),
    ]));

    let p = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(fg))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(" EQ Tuner ", theme.accent())),
        );
    f.render_widget(p, popup);
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
        Row {
            label: "EQ Low",
            value: format!("{:+.0} dB", eq.low_db),
            bar_pct: ((eq.low_db + 12.0) / 24.0) as f64,
        },
        Row {
            label: "EQ Mid",
            value: format!("{:+.0} dB", eq.mid_db),
            bar_pct: ((eq.mid_db + 12.0) / 24.0) as f64,
        },
        Row {
            label: "EQ High",
            value: format!("{:+.0} dB", eq.high_db),
            bar_pct: ((eq.high_db + 12.0) / 24.0) as f64,
        },
        Row {
            label: "EQ Preset",
            value: preset_name.to_string(),
            bar_pct: app.eq_preset_idx as f64 / (crate::eq::PRESETS.len() - 1).max(1) as f64,
        },
        Row {
            label: "Volume",
            value: format!("{}%", vol_pct),
            bar_pct: (app.player.volume() / 1.5) as f64,
        },
        Row {
            label: "Crossfade",
            value: format!("{:.1}s", xf),
            bar_pct: (xf / 10.0) as f64,
        },
        Row {
            label: "Viz Sens",
            value: format!("×{:.1}", sens),
            bar_pct: ((sens - 0.1) / 2.9) as f64,
        },
        Row {
            label: "Speed",
            value: format!("{:.2}×", app.player.speed()),
            bar_pct: ((app.player.speed() - 0.5) / 2.0) as f64,
        },
    ];

    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);
    let accent = parse_color(&theme.colors.accent);
    let selected_bg = parse_color(&theme.colors.primary);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " Audio Panel — ↑↓ select  ←→ adjust  e/Esc close ",
            theme.accent(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let bar_width = inner.width.saturating_sub(22) as usize;

    for (i, row) in rows.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let selected = i == app.audio_panel_row;
        let row_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };

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
            Span::styled(
                bar,
                if selected {
                    Style::default().fg(accent)
                } else {
                    Style::default().fg(parse_color(&theme.colors.muted))
                },
            ),
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
        let hint_area = Rect {
            x: inner.x,
            y: hint_y,
            width: inner.width,
            height: 1,
        };
        let hint = Paragraph::new(Line::from(vec![Span::styled(
            "  ← / → to adjust  |  ↑↓ to navigate  |  e or Esc to close",
            Style::default().fg(muted),
        )]));
        f.render_widget(hint, hint_area);
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
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

fn render_main(f: &mut Frame, area: Rect, app: &mut App) {
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

fn render_library(f: &mut Frame, area: Rect, app: &mut App) {
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
                    // #66: highlight the matched substring inside the row.
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

    // Track art_area so the overlay renderer can position Kitty/iTerm2 images.
    app.layout.art_area = art_area;

    if app.player.current().is_some() {
        if let Some(img) = &app.album_art {
            if app.art_picker.protocol == crate::album_art::Protocol::Blocks {
                // Half-block rendering lives inside ratatui — render it here.
                album_art::render_blocks(f, art_area, img);
            }
            // Kitty / iTerm2 are emitted via render_overlay_art() after terminal.draw().
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
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
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
                std::iter::repeat_n(block_chars[idx], bar_w).collect()
            } else {
                " ".repeat(bar_w)
            };
            let color = if val > 0.75 {
                accent
            } else if val > 0.4 {
                primary
            } else {
                secondary
            };
            spans.push(Span::styled(ch, Style::default().fg(color)));
            if i + 1 < n_bars {
                spans.push(Span::raw(" "));
            }
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
        );
    }

    // Bottom row: label
    let label = Line::from(Span::styled("▶ NOW PLAY", Style::default().fg(muted)));
    f.render_widget(
        Paragraph::new(label).alignment(Alignment::Center),
        Rect {
            x: area.x,
            y: area.y + h as u16 - 1,
            width: area.width,
            height: 1,
        },
    );
}

fn build_eq_row(eq: &crate::eq::EqState, theme: &Theme) -> Line<'static> {
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

/// Split `text` into spans, applying `match_color` to occurrences of `needle`
/// (case-insensitive) and `base_color` to the rest. Used by the search filter to
/// highlight matched substrings (#66).
fn highlight_match(
    text: &str,
    needle: &str,
    base: ratatui::style::Color,
    hit: ratatui::style::Color,
) -> Vec<Span<'static>> {
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

fn build_progress(
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

fn render_status(f: &mut Frame, area: Rect, app: &App) {
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

fn render_lastfm(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::Clear;
    let w = (area.width as i32 - 8).max(50) as u16;
    let h = (area.height as i32 - 4).max(20) as u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            " Last.fm dashboard ",
            Style::default()
                .fg(parse_color(&app.theme.colors.accent))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Recent scrobbles",
            Style::default()
                .fg(parse_color(&app.theme.colors.accent))
                .add_modifier(Modifier::BOLD),
        )),
    ];
    if app.lastfm_panel_recent.is_empty() {
        lines.push(Line::from(" (loading or no scrobbles yet)"));
    } else {
        for t in &app.lastfm_panel_recent {
            lines.push(Line::from(format!(" • {} — {}", t.artist, t.title)));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Top artists (last month)",
        Style::default()
            .fg(parse_color(&app.theme.colors.accent))
            .add_modifier(Modifier::BOLD),
    )));
    if app.lastfm_panel_top_artists.is_empty() {
        lines.push(Line::from(" (loading)"));
    } else {
        for (i, a) in app.lastfm_panel_top_artists.iter().enumerate() {
            lines.push(Line::from(format!(
                " {:>2}. {} ({} plays)",
                i + 1,
                a.name,
                a.playcount
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [Ctrl+L] close ",
        Style::default().fg(parse_color(&app.theme.colors.muted)),
    )));

    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(true))
        .title(Span::styled(" Last.fm (#63) ", app.theme.accent()));
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

pub(crate) fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn render_stats(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::Clear;
    let stats = crate::stats::PlaybackStats::compute(&app.play_history, &app.library, 10);

    let w = (area.width as i32 - 8).max(50) as u16;
    let h = (area.height as i32 - 4).max(20) as u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let listen_h = stats.estimated_listen_secs / 3600;
    let listen_m = (stats.estimated_listen_secs % 3600) / 60;

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            format!(
                " Listening stats — total {}h{:02}m, {} plays across {} tracks ",
                listen_h, listen_m, stats.total_plays, stats.unique_tracks
            ),
            Style::default()
                .fg(parse_color(&app.theme.colors.accent))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Top tracks",
            Style::default()
                .fg(parse_color(&app.theme.colors.accent))
                .add_modifier(Modifier::BOLD),
        )),
    ];
    for (i, (display, _path, count)) in stats.top_tracks.iter().enumerate() {
        lines.push(Line::from(format!(
            " {:>2}. {} ({} plays)",
            i + 1,
            display,
            count
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Top artists",
        Style::default()
            .fg(parse_color(&app.theme.colors.accent))
            .add_modifier(Modifier::BOLD),
    )));
    for (i, (artist, count)) in stats.top_artists.iter().enumerate() {
        lines.push(Line::from(format!(
            " {:>2}. {} ({} plays)",
            i + 1,
            artist,
            count
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Recently played",
        Style::default()
            .fg(parse_color(&app.theme.colors.accent))
            .add_modifier(Modifier::BOLD),
    )));
    for (i, (display, _)) in stats.recent_tracks.iter().enumerate() {
        lines.push(Line::from(format!(" {:>2}. {}", i + 1, display)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [Ctrl+S] close ",
        Style::default().fg(parse_color(&app.theme.colors.muted)),
    )));

    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(true))
        .title(Span::styled(" Stats (#64) ", app.theme.accent()));
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

/// Format a unix timestamp as a short relative duration ago — e.g. "5m", "3h",
/// "2d", "3w". `0` (never played) returns "never" so callers can branch on it.
/// Used by the playlist browser row hints (#84).
fn relative_time(ts: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
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

fn render_tag_editor(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 60.min(area.width.saturating_sub(4)).max(30);
    let h = 14.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " Edit Tags — Tab/↑↓ field  Enter save  Esc cancel ",
            theme.accent(),
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let labels = ["Title", "Artist", "Album", "Genre", "Year"];
    let accent = parse_color(&theme.colors.accent);
    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);

    for (i, label) in labels.iter().enumerate() {
        let y = inner.y + 1 + (i as u16 * 2);
        if y >= inner.y + inner.height {
            break;
        }
        let selected = i == app.tag_editor_row;
        let lbl_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted)
        };

        let val = &app.tag_editor_fields[i];
        let val_display = if selected {
            format!("{val}█")
        } else if val.is_empty() {
            "<empty>".to_string()
        } else {
            val.clone()
        };

        let val_style = if selected {
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };

        let line = Line::from(vec![
            Span::styled(format!(" {:<7}: ", label), lbl_style),
            Span::styled(val_display, val_style),
        ]);
        let r = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), r);
    }
}

fn render_radio_custom_modal(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 64.min(area.width.saturating_sub(4)).max(32);
    let h = 10.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " ✨ Adicionar Rádio — Tab/↑↓ campo · Enter salvar · Esc cancelar ",
            theme.accent(),
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let labels = ["Nome da Estação", "URL do Stream", "Gênero / Tags"];
    let placeholders = [
        "Ex: Rádio Retrô FM",
        "Ex: https://stream.exemplo.com/live.mp3",
        "Ex: synthwave, 80s, instrumental",
    ];
    let accent = parse_color(&theme.colors.accent);
    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);

    for (i, label) in labels.iter().enumerate() {
        let y = inner.y + 1 + (i as u16 * 2);
        if y >= inner.y + inner.height {
            break;
        }
        let selected = i == app.radio_custom_field_idx;
        let lbl_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted)
        };

        let val = &app.radio_custom_fields[i];
        let (val_display, val_style) = if selected {
            (
                format!("{val}█"),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            )
        } else if val.is_empty() {
            (placeholders[i].to_string(), Style::default().fg(muted))
        } else {
            (val.clone(), Style::default().fg(fg))
        };

        let line = Line::from(vec![
            Span::styled(format!(" {:<16}: ", label), lbl_style),
            Span::styled(val_display, val_style),
        ]);
        let r = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), r);
    }
}

fn render_radio_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 80.min(area.width.saturating_sub(2)).max(40);
    let h = 24.min(area.height.saturating_sub(2)).max(12);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let title =
        " 📻 Online Radio Hub — Tab tab · Enter play · a enqueue · f fav (♥) · +/N add · / search · Esc close ";
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(title, theme.accent()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab header
            Constraint::Length(2), // Search bar
            Constraint::Min(4),    // Station list
        ])
        .split(inner);

    let accent = parse_color(&theme.colors.accent);
    let primary = parse_color(&theme.colors.primary);
    let secondary = parse_color(&theme.colors.secondary);
    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);

    // Tab Header
    let is_curated = app.radio_tab == crate::radio_browser::RadioTab::Curated;
    let tab_curated_style = if is_curated {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(muted)
    };
    let tab_search_style = if !is_curated {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(muted)
    };

    let tabs_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            if is_curated {
                "[★ Curated Stations]"
            } else {
                " ★ Curated Stations "
            },
            tab_curated_style,
        ),
        Span::raw("   "),
        Span::styled(
            if !is_curated {
                "[🔍 Search Radio-Browser (+40k)]"
            } else {
                " 🔍 Search Radio-Browser (+40k) "
            },
            tab_search_style,
        ),
    ]);
    f.render_widget(Paragraph::new(tabs_line), chunks[0]);

    // Search Bar
    if app.radio_tab == crate::radio_browser::RadioTab::Search {
        let cursor = if app.radio_search_editing { "█" } else { "" };
        let search_text = if app.radio_search_query.is_empty() && !app.radio_search_editing {
            "  (press / to type search query, Enter to search)".to_string()
        } else {
            format!("  🔍 {}{cursor}", app.radio_search_query)
        };
        let search_style = if app.radio_search_editing {
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted)
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(search_text, search_style)])),
            chunks[1],
        );
    }

    // Station list
    let stations: &[crate::radio_browser::RadioStation] = match app.radio_tab {
        crate::radio_browser::RadioTab::Curated => &app.radio_curated_list,
        crate::radio_browser::RadioTab::Search => &app.radio_search_results,
    };

    let list_area = if app.radio_tab == crate::radio_browser::RadioTab::Search {
        chunks[2]
    } else {
        Rect {
            x: chunks[1].x,
            y: chunks[1].y,
            width: chunks[1].width,
            height: chunks[1].height + chunks[2].height,
        }
    };

    if stations.is_empty() {
        let empty_msg = if app.radio_tab == crate::radio_browser::RadioTab::Search {
            if app.radio_search_rx.is_some() {
                "  Searching online stations…"
            } else {
                "  No stations found. Press / to search by genre (e.g. jazz, lofi, rock, brazil) or name."
            }
        } else {
            "  No curated stations available."
        };
        f.render_widget(
            Paragraph::new(Span::styled(empty_msg, Style::default().fg(muted))),
            list_area,
        );
        return;
    }

    let items_per_page = list_area.height as usize;
    let scroll_offset = if app.radio_row >= items_per_page {
        app.radio_row - items_per_page + 1
    } else {
        0
    };

    let mut lines = Vec::new();
    for (i, st) in stations
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(items_per_page)
    {
        let selected = i == app.radio_row;
        let prefix = if selected { "▶ " } else { "  " };
        let is_fav = app.ratings.is_favorite(&std::path::PathBuf::from(&st.url));
        let fav_icon = if is_fav { "♥ " } else { "  " };

        let name_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };

        let country = st.country.as_deref().unwrap_or("World");
        let bitrate_str = st
            .bitrate
            .map(|b| format!("{b}k"))
            .unwrap_or_else(|| "128k".into());
        let tags_trimmed = if st.tags.len() > 24 {
            format!("{}…", &st.tags[..23])
        } else {
            st.tags.clone()
        };

        let row_line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(accent)),
            Span::styled(fav_icon, Style::default().fg(accent)),
            Span::styled(format!("{:<26} ", st.name), name_style),
            Span::styled(format!(" {:<12} ", country), Style::default().fg(secondary)),
            Span::styled(
                format!(" {:<6} ", bitrate_str),
                Style::default().fg(primary),
            ),
            Span::styled(format!(" {}", tags_trimmed), Style::default().fg(muted)),
        ]);
        lines.push(row_line);
    }

    f.render_widget(Paragraph::new(lines), list_area);
}

fn render_radio_view(f: &mut Frame, area: Rect, app: &App) {
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

fn render_lyrics_modal(f: &mut Frame, area: Rect, app: &mut App) {
    let w = 80.min(area.width.saturating_sub(4));
    let h = 24.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let primary = parse_color(&app.theme.colors.primary);
    let accent = parse_color(&app.theme.colors.accent);
    let muted = parse_color(&app.theme.colors.muted);
    let fg = parse_color(&app.theme.colors.foreground);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(true))
        .title(Span::styled(
            " 🎤 Letras Sincronizadas / Karaoke [y/Esc: fechar · Enter: ir ao verso · k/j: rolar · c: auto-scroll · r: buscar] ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let elapsed = app.player.elapsed();

    if let Some(lyrics) = &app.lyrics {
        if lyrics.lines.is_empty() {
            let p = Paragraph::new("Letra vazia.").style(Style::default().fg(muted));
            f.render_widget(p, inner);
            return;
        }

        let cur_idx = lyrics.current_index(elapsed);
        let total_lines = lyrics.lines.len();
        let visible_lines = inner.height as usize;

        let active_row = if app.lyrics_auto_scroll {
            if let Some(ci) = cur_idx {
                app.lyrics_scroll = ci;
                ci
            } else {
                app.lyrics_scroll
            }
        } else {
            app.lyrics_scroll
        };

        let half = visible_lines / 2;
        let start_idx = active_row.saturating_sub(half);
        let end_idx = (start_idx + visible_lines).min(total_lines);

        let mut lines: Vec<Line> = Vec::new();
        for i in start_idx..end_idx {
            let line = &lyrics.lines[i];
            let is_current = Some(i) == cur_idx;
            let is_cursor = i == app.lyrics_scroll;
            let time_str = format_duration(line.at);

            let (prefix, text_style, time_style) = if is_current {
                (
                    " ▶ ",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    Style::default().fg(primary).add_modifier(Modifier::BOLD),
                )
            } else if is_cursor && !app.lyrics_auto_scroll {
                (
                    " • ",
                    Style::default().fg(fg).add_modifier(Modifier::UNDERLINED),
                    Style::default().fg(muted),
                )
            } else if cur_idx.map(|c| i < c).unwrap_or(false) {
                (
                    "   ",
                    Style::default().fg(muted),
                    Style::default().fg(muted),
                )
            } else {
                ("   ", Style::default().fg(fg), Style::default().fg(muted))
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, text_style),
                Span::styled(format!("[{}] ", time_str), time_style),
                Span::styled(&line.text, text_style),
            ]));
        }

        let p = Paragraph::new(lines);
        f.render_widget(p, inner);
    } else {
        let current_display = app
            .player
            .current()
            .map(|t| t.display())
            .unwrap_or_else(|| "Nenhuma música tocando".to_string());

        let msg = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Música: ", Style::default().fg(muted)),
                Span::styled(
                    current_display,
                    Style::default().fg(fg).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Nenhuma letra sincronizada encontrada para esta faixa.",
                Style::default().fg(muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  • Pressione 'r' para buscar novamente no LRCLIB.",
                Style::default().fg(accent),
            )),
            Line::from(Span::styled(
                "  • Ou coloque um arquivo .lrc com mesmo nome junto ao arquivo de áudio.",
                Style::default().fg(fg),
            )),
        ];
        f.render_widget(Paragraph::new(msg), inner);
    }
}
