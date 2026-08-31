use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::collections::HashSet;

use crate::{
    app::{App, SpotifyTab},
    theme::parse_color,
    ui::util::{format_duration, relative_time},
};

pub fn render_track_info(f: &mut Frame, area: Rect, app: &App) {
    let Some(track) = app.player.current() else {
        return;
    };
    let snapshot = app
        .track_info
        .as_ref()
        .filter(|info| info.path == track.path);
    let is_local = snapshot.map(|info| info.is_local).unwrap_or(false);
    let meta = snapshot.map(|info| info.meta.clone()).unwrap_or_default();

    let muted = parse_color(&app.theme.colors.muted);
    let fg = parse_color(&app.theme.colors.foreground);
    let accent = parse_color(&app.theme.colors.accent);

    let section_header = |title: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("── {} ", title),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled("─".repeat(40), Style::default().fg(muted)),
        ])
    };

    let row = |label: &str, value: String| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {:<18}", label), Style::default().fg(muted)),
            Span::styled(value, Style::default().fg(fg)),
        ])
    };
    let dash = "—".to_string();

    let dur_str = track
        .duration
        .or(meta.duration)
        .map(|d| {
            let s = d.as_secs();
            format!("{:02}:{:02}", s / 60, s % 60)
        })
        .unwrap_or_else(|| {
            if !is_local {
                "Live Stream".to_string()
            } else {
                dash.clone()
            }
        });

    let rate_str = meta
        .sample_rate
        .map(|r| format!("{} Hz ({:.1} kHz)", r, r as f32 / 1000.0))
        .unwrap_or_else(|| {
            let r = app.player.current_sample_rate;
            if r > 0 {
                format!("{} Hz ({:.1} kHz)", r, r as f32 / 1000.0)
            } else {
                dash.clone()
            }
        });

    let chans_str = meta
        .channels
        .map(|c| {
            if c == 2 {
                "Stereo (2ch)".into()
            } else if c == 1 {
                "Mono (1ch)".into()
            } else {
                format!("{c} canais")
            }
        })
        .unwrap_or_else(|| {
            let c = app.player.current_channels;
            if c == 2 {
                "Stereo (2ch)".into()
            } else if c == 1 {
                "Mono (1ch)".into()
            } else {
                format!("{c} canais")
            }
        });

    let codec_str = meta.codec.unwrap_or_else(|| {
        let c = app.player.current_codec.clone();
        if c != "—" && !c.is_empty() {
            c
        } else {
            let p = track.path.to_string_lossy();
            if p.contains(".mp3") {
                "MP3".into()
            } else if p.contains(".flac") {
                "FLAC".into()
            } else if p.contains(".m4a") || p.contains(".aac") {
                "AAC".into()
            } else if p.contains(".opus") {
                "Opus".into()
            } else if p.contains(".ogg") {
                "Ogg Vorbis".into()
            } else {
                "Audio Stream".into()
            }
        }
    });

    let path_str = track.path.to_string_lossy().to_string();
    let (source_type, file_size_str) = if is_local {
        let size = snapshot.map(|info| info.file_size).unwrap_or(0);
        let sz_str = if size > 1024 * 1024 {
            format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
        } else if size > 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else {
            format!("{size} B")
        };
        ("Arquivo Local", sz_str)
    } else if path_str.starts_with("https://www.youtube.com")
        || path_str.starts_with("https://youtu.be")
        || path_str.starts_with("ytsearch:")
    {
        let cached = snapshot.map(|info| info.youtube_cached).unwrap_or(false);
        if cached {
            ("YouTube (Cache Local)", "Em cache no disco".into())
        } else {
            ("YouTube (Streaming)", "Stream online".into())
        }
    } else if path_str.starts_with("http") {
        ("Rádio Online / Web Stream", "Transmissão contínua".into())
    } else {
        ("Streaming", dash.clone())
    };

    let lrc_status = if snapshot.map(|info| info.has_local_lrc).unwrap_or(false) {
        "Disponível (.lrc local)"
    } else if app.lyrics.is_some() {
        "Disponível (LRCLIB online)"
    } else {
        "Não disponível"
    };

    let rg_str = if let Some(t_db) = track.replaygain_track_db {
        format!("{t_db:+.2} dB (escala: {:.2}x)", app.player.rg_scale)
    } else if let Some(a_db) = track.replaygain_album_db {
        format!(
            "{a_db:+.2} dB (album gain, escala: {:.2}x)",
            app.player.rg_scale
        )
    } else {
        format!("Desativado (escala: {:.2}x)", app.player.rg_scale)
    };

    let lines: Vec<Line> = vec![
        section_header("Metadados"),
        row("Título", meta.title.unwrap_or_else(|| track.title.clone())),
        row(
            "Artista",
            meta.artist
                .or_else(|| track.artist.clone())
                .unwrap_or_else(|| dash.clone()),
        ),
        row(
            "Álbum",
            meta.album
                .or_else(|| track.album.clone())
                .unwrap_or_else(|| dash.clone()),
        ),
        row(
            "Ano",
            meta.year
                .or_else(|| track.year.clone())
                .unwrap_or_else(|| dash.clone()),
        ),
        row(
            "Gênero",
            meta.genre
                .or_else(|| track.genre.clone())
                .unwrap_or_else(|| dash.clone()),
        ),
        Line::from(""),
        section_header("Ficha Técnica de Áudio"),
        row("Codec", codec_str),
        row("Taxa de Amostragem", rate_str),
        row("Canais", chans_str),
        row("Duração", dur_str),
        row("ReplayGain", rg_str),
        row("Velocidade", format!("{:.2}x", app.player.speed)),
        row("Letras (LRC)", lrc_status.to_string()),
        Line::from(""),
        section_header("Origem & Armazenamento"),
        row("Tipo de Fonte", source_type.to_string()),
        row("Tamanho", file_size_str),
        row("Localização", path_str),
    ];

    let w = 76.min(area.width.saturating_sub(2));
    let h = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
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
                " ℹ Inspetor Técnico da Faixa — Pressione qualquer tecla ",
                app.theme.accent(),
            )),
    );
    f.render_widget(p, popup);
}

pub fn render_playlist_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let entries = &app.playlist_browser_entries;

    let w = 60.min(area.width.saturating_sub(4));
    let h = (entries.len() as u16 + 5)
        .max(6)
        .min(area.height.saturating_sub(2));
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

pub fn render_profile_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let profiles = &app.profiles;

    let w = 60.min(area.width.saturating_sub(4));
    let h = (profiles.len() as u16 + 6)
        .max(7)
        .min(area.height.saturating_sub(2));
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

pub fn render_spotify_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let accent = parse_color(&theme.colors.accent);
    let muted = parse_color(&theme.colors.muted);
    let fg = parse_color(&theme.colors.foreground);
    let bg = parse_color(&theme.colors.background);
    let secondary = parse_color(&theme.colors.secondary);

    let w = 80.min(area.width.saturating_sub(2));
    let h = area.height.saturating_sub(2);
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

pub fn render_subsonic_browser(f: &mut Frame, area: Rect, app: &App) {
    use crate::app::types::SubsonicTab;
    let theme = &app.theme;
    let w = 78.min(area.width.saturating_sub(2));
    let h = 22.min(area.height.saturating_sub(2));
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

    let tab_line = Line::from(vec![
        Span::styled(
            if app.subsonic_browser_tab == SubsonicTab::Search {
                " [🔍 Busca] "
            } else {
                "  🔍 Busca  "
            },
            if app.subsonic_browser_tab == SubsonicTab::Search {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
        Span::styled(
            if app.subsonic_browser_tab == SubsonicTab::RecentAlbums {
                " [💿 Álbuns Recentes] "
            } else {
                "  💿 Álbuns Recentes  "
            },
            if app.subsonic_browser_tab == SubsonicTab::RecentAlbums {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
        Span::styled(
            if app.subsonic_browser_tab == SubsonicTab::Playlists {
                " [📑 Playlists] "
            } else {
                "  📑 Playlists  "
            },
            if app.subsonic_browser_tab == SubsonicTab::Playlists {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
        Span::styled(
            if app.subsonic_browser_tab == SubsonicTab::Random {
                " [🎲 Aleatórias] "
            } else {
                "  🎲 Aleatórias  "
            },
            if app.subsonic_browser_tab == SubsonicTab::Random {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(muted)
            },
        ),
    ]);

    let hint = Line::from(vec![
        Span::styled(" Tab", Style::default().fg(accent)),
        Span::styled(" abas  ", Style::default().fg(muted)),
        Span::styled("/", Style::default().fg(accent)),
        Span::styled(" buscar  ", Style::default().fg(muted)),
        Span::styled("Enter", Style::default().fg(accent)),
        Span::styled(" tocar/abrir  ", Style::default().fg(muted)),
        Span::styled("a", Style::default().fg(accent)),
        Span::styled(" fila  ", Style::default().fg(muted)),
        Span::styled("Esc", Style::default().fg(accent)),
        Span::styled(" fechar", Style::default().fg(muted)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " ☁️ Subsonic / Navidrome Cloud Streaming ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(hint);
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if !app.config.subsonic.is_configured() {
        let msg = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  ⚠️ Servidor Subsonic / Navidrome não configurado.",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Configure no seu arquivo config.toml:",
                Style::default().fg(fg),
            )),
            Line::from(Span::styled("  [subsonic]", Style::default().fg(secondary))),
            Line::from(Span::styled(
                "  server_url = \"http://seu-servidor:4533\"",
                Style::default().fg(muted),
            )),
            Line::from(Span::styled(
                "  username = \"seu_usuario\"",
                Style::default().fg(muted),
            )),
            Line::from(Span::styled(
                "  password = \"sua_senha\"",
                Style::default().fg(muted),
            )),
        ];
        f.render_widget(Paragraph::new(msg), inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    // Search bar
    let query_display = if app.subsonic_browser_query_editing {
        format!(" 🔍 Busca: {}█", app.subsonic_browser_query)
    } else if app.subsonic_browser_query.is_empty() {
        " Pressione / para buscar músicas no seu Navidrome/Subsonic…".to_string()
    } else {
        format!(" 🔍 Busca: {}", app.subsonic_browser_query)
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            query_display,
            Style::default().fg(if app.subsonic_browser_query_editing {
                fg
            } else {
                muted
            }),
        )),
        chunks[0],
    );

    // Tab bar
    f.render_widget(Paragraph::new(tab_line), chunks[1]);

    // Results area
    let list_area = chunks[2];
    match app.subsonic_browser_tab {
        SubsonicTab::Search | SubsonicTab::Random => {
            let results = &app.subsonic_browser_results;
            if results.is_empty() {
                let msg = if app.subsonic_browser_tab == SubsonicTab::Random {
                    "  Carregando músicas aleatórias…"
                } else {
                    "  Nenhum resultado. Digite o termo de busca e pressione Enter."
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
                        let selected = i == app.subsonic_browser_row;
                        let dur = t
                            .duration
                            .map(|d| {
                                let s = d.as_secs();
                                format!("{:02}:{:02}", s / 60, s % 60)
                            })
                            .unwrap_or_default();
                        let label = format!(
                            " {} {:<38} {:<24} {}",
                            if selected { "▶" } else { " " },
                            t.title.chars().take(36).collect::<String>(),
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
                state.select(Some(app.subsonic_browser_row));
                f.render_stateful_widget(
                    List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
                    list_area,
                    &mut state,
                );
            }
        }
        SubsonicTab::RecentAlbums => {
            if app.subsonic_browser_albums.is_empty() {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "  Carregando álbuns do servidor…",
                        Style::default().fg(muted),
                    )),
                    list_area,
                );
            } else {
                let items: Vec<ListItem> = app
                    .subsonic_browser_albums
                    .iter()
                    .enumerate()
                    .map(|(i, album)| {
                        let selected = i == app.subsonic_browser_row;
                        let label = format!(
                            " {} {:<40} {:<24} ({} faixas)",
                            if selected { "▶" } else { " " },
                            album.display_title().chars().take(38).collect::<String>(),
                            album
                                .artist
                                .as_deref()
                                .unwrap_or("")
                                .chars()
                                .take(22)
                                .collect::<String>(),
                            album.song_count.unwrap_or(0),
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
                state.select(Some(app.subsonic_browser_row));
                f.render_stateful_widget(
                    List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
                    list_area,
                    &mut state,
                );
            }
        }
        SubsonicTab::Playlists => {
            if app.subsonic_browser_playlists.is_empty() {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "  Carregando playlists do servidor…",
                        Style::default().fg(muted),
                    )),
                    list_area,
                );
            } else {
                let items: Vec<ListItem> = app
                    .subsonic_browser_playlists
                    .iter()
                    .enumerate()
                    .map(|(i, pl)| {
                        let selected = i == app.subsonic_browser_row;
                        let label = format!(
                            " {} {:<48} ({} faixas)",
                            if selected { "▶" } else { " " },
                            pl.name.chars().take(46).collect::<String>(),
                            pl.song_count.unwrap_or(0),
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
                state.select(Some(app.subsonic_browser_row));
                f.render_stateful_widget(
                    List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
                    list_area,
                    &mut state,
                );
            }
        }
    }
}

pub fn render_vault_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 78.min(area.width.saturating_sub(2));
    let h = 22.min(area.height.saturating_sub(2));
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

    let hint = Line::from(vec![
        Span::styled(" /", Style::default().fg(accent)),
        Span::styled(" buscar  ", Style::default().fg(muted)),
        Span::styled("Enter", Style::default().fg(accent)),
        Span::styled(" tocar  ", Style::default().fg(muted)),
        Span::styled("a", Style::default().fg(accent)),
        Span::styled(" enfileirar  ", Style::default().fg(muted)),
        Span::styled("Esc", Style::default().fg(accent)),
        Span::styled(" fechar", Style::default().fg(muted)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " ☁️ Cloud Audio Vault — Streaming Comunitário Direto ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(hint);
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Search bar
            Constraint::Min(1),    // Results list
        ])
        .split(inner);

    // Search bar
    let query_display = if app.vault_query_editing {
        format!(" 🔍 Busca no Vault: {}█", app.vault_query)
    } else if app.vault_query.is_empty() {
        " Pressione / para pesquisar no catálogo em nuvem do Vault…".to_string()
    } else {
        format!(" 🔍 Busca no Vault: {}", app.vault_query)
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            query_display,
            Style::default().fg(if app.vault_query_editing { fg } else { muted }),
        )),
        chunks[0],
    );

    // Results
    let list_area = chunks[1];
    let results = &app.vault_results;
    if results.is_empty() {
        let msg = "  Nenhuma faixa encontrada no catálogo. Digite um termo e tecle Enter.";
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(muted))),
            list_area,
        );
    } else {
        let items: Vec<ListItem> = results
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let selected = i == app.vault_row;
                let dur = t
                    .duration
                    .map(|d| {
                        let s = d.as_secs();
                        format!("{:02}:{:02}", s / 60, s % 60)
                    })
                    .unwrap_or_default();
                let label = format!(
                    " {} {:<38} {:<24} {}",
                    if selected { "▶" } else { " " },
                    t.title.chars().take(36).collect::<String>(),
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
        state.select(Some(app.vault_row));
        f.render_stateful_widget(
            List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
            list_area,
            &mut state,
        );
    }
}

pub fn render_device_selector(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 70.min(area.width.saturating_sub(4));
    let h = (app.device_list.len() as u16 + 4)
        .max(6)
        .min(area.height.saturating_sub(2));
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

pub fn render_eq_tuner(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let eq = app.player.eq().snapshot();
    let curve_h = 7usize;
    let w = 84.min(area.width.saturating_sub(2));
    let h = 26.min(area.height.saturating_sub(2));
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

    let node_cols: Vec<usize> = crate::eq::BAND_FREQS
        .iter()
        .map(|&freq| freq_to_col(freq))
        .collect();

    let grid_cols: HashSet<usize> = [31.25f32, 125.0, 500.0, 2000.0, 8000.0, 16000.0]
        .iter()
        .map(|&freq| freq_to_col(freq))
        .collect();

    let curve_rows: Vec<usize> = (0..curve_w)
        .map(|col| {
            let t = col as f32 / (curve_w - 1).max(1) as f32;
            let freq = 20.0f32 * (1000.0f32).powf(t);
            let mut total = 0.0f32;
            for i in 0..crate::eq::NUM_BANDS {
                let fc = crate::eq::BAND_FREQS[i];
                let g = eq.bands[i];
                if g.abs() > 0.05 {
                    let ratio = (freq / fc).ln();
                    let resp = (-0.5 * (ratio / 0.55).powi(2)).exp();
                    total += g * resp;
                }
            }
            let total = total.clamp(-12.0, 12.0);
            let row = ((12.0 - total) / 24.0 * (curve_h - 1) as f32).round() as usize;
            row.min(curve_h - 1)
        })
        .collect();

    let zero_row = (curve_h - 1) / 2;

    let mut lines: Vec<Line> = Vec::new();

    // 1. Curve rows
    for row in 0..curve_h {
        let db = 12.0 - row as f32 * 24.0 / (curve_h - 1) as f32;
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
            let is_vert = if col > 0 && col < curve_w - 1 {
                let prev = curve_rows[col - 1] as isize;
                let next = curve_rows[col + 1] as isize;
                (prev < r && r < next) || (next < r && r < prev)
            } else {
                false
            };

            let in_boost = cr < zr && r > cr && r < zr;
            let in_cut = cr > zr && r > zr && r < cr;

            let is_selected_node = node_cols
                .get(app.eq_tuner_band)
                .map(|&c| c == col && is_on_curve)
                .unwrap_or(false);

            let is_any_node = node_cols.iter().position(|&c| c == col && is_on_curve);
            let is_grid_col = grid_cols.contains(&col);

            let span = if is_selected_node {
                Span::styled(
                    "●",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                )
            } else if is_any_node.is_some() {
                Span::styled("•", Style::default().fg(secondary))
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
                Span::styled("┆", Style::default().fg(muted))
            } else {
                Span::raw(" ")
            };
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }

    // 2. Frequency axis labels
    {
        let mut freq_row = vec![' '; curve_w];
        for (i, label) in crate::eq::BAND_LABELS.iter().enumerate() {
            let col = node_cols[i];
            let clean_label = label.trim_end_matches("Hz");
            let col_start = col
                .saturating_sub(clean_label.len() / 2)
                .min(curve_w.saturating_sub(clean_label.len()));
            for (ci, ch) in clean_label.chars().enumerate() {
                if col_start + ci < curve_w && freq_row[col_start + ci] == ' ' {
                    freq_row[col_start + ci] = ch;
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

    // 3. 10 Vertical Faders
    let fader_height = 5usize;
    // Row 0: DB Values
    let mut db_spans: Vec<Span> = vec![Span::styled(" Gain: ", Style::default().fg(muted))];
    for (i, &gain) in eq.bands.iter().enumerate() {
        let is_sel = i == app.eq_tuner_band;
        let style = if is_sel {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };
        db_spans.push(Span::styled(format!("{:>5.0}dB ", gain), style));
    }
    lines.push(Line::from(db_spans));

    // Rows 1..5: Vertical fader tracks
    for fader_row in 0..fader_height {
        let mut track_spans: Vec<Span> = vec![Span::styled(
            if fader_row == fader_height / 2 {
                "  0dB ├"
            } else {
                "      │"
            },
            Style::default().fg(muted),
        )];
        for (i, &gain) in eq.bands.iter().enumerate() {
            let is_sel = i == app.eq_tuner_band;
            let knob_row = (((12.0 - gain) / 24.0) * (fader_height - 1) as f32).round() as usize;
            let center_row = fader_height / 2;

            let cell = if fader_row == knob_row {
                "  [●]  "
            } else if fader_row == center_row {
                "  ─┼─  "
            } else {
                "   │   "
            };

            let style = if is_sel && fader_row == knob_row {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else if is_sel {
                Style::default().fg(accent)
            } else {
                Style::default().fg(muted)
            };
            track_spans.push(Span::styled(cell, style));
        }
        track_spans.push(Span::styled("┤", Style::default().fg(muted)));
        lines.push(Line::from(track_spans));
    }

    // Row 6: Band frequency labels
    let mut label_spans: Vec<Span> = vec![Span::styled(" Freq: ", Style::default().fg(muted))];
    for (i, label) in crate::eq::BAND_LABELS.iter().enumerate() {
        let is_sel = i == app.eq_tuner_band;
        let style = if is_sel {
            Style::default()
                .fg(bg)
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(secondary)
        };
        label_spans.push(Span::styled(format!(" {:^5} ", label), style));
        label_spans.push(Span::raw(""));
    }
    lines.push(Line::from(label_spans));

    lines.push(Line::from(""));

    // 4. Preset buttons
    let current = crate::eq::PRESETS.iter().position(|(_, s)| {
        s.bands
            .iter()
            .zip(eq.bands.iter())
            .all(|(a, b)| (a - b).abs() < 0.1)
    });
    let mut preset_spans: Vec<Span> = vec![Span::styled(" Presets: ", Style::default().fg(muted))];
    for (i, (name, _)) in crate::eq::PRESETS.iter().enumerate().take(8) {
        let active = current == Some(i);
        let style = if active {
            Style::default()
                .fg(bg)
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted)
        };
        preset_spans.push(Span::styled(format!(" {} ", name), style));
        preset_spans.push(Span::raw(" "));
    }
    lines.push(Line::from(preset_spans));

    // 5. Controls hint
    lines.push(Line::from(vec![
        Span::styled(" ← → ", Style::default().fg(accent)),
        Span::styled("banda  ", Style::default().fg(muted)),
        Span::styled("↑↓ ", Style::default().fg(accent)),
        Span::styled("ganho (±1dB)  ", Style::default().fg(muted)),
        Span::styled("0 ", Style::default().fg(accent)),
        Span::styled("preset  ", Style::default().fg(muted)),
        Span::styled("r ", Style::default().fg(accent)),
        Span::styled("flat  ", Style::default().fg(muted)),
        Span::styled("Ctrl+S ", Style::default().fg(accent)),
        Span::styled("salvar  ", Style::default().fg(muted)),
        Span::styled("Esc ", Style::default().fg(accent)),
        Span::styled("fechar", Style::default().fg(muted)),
    ]));

    let p = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(fg))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(
                    " 🎚️ Equalizador Gráfico de 10 Bandas (EQ Tuner) ",
                    theme.accent(),
                )),
        );
    f.render_widget(p, popup);
}

pub fn render_audio_panel(f: &mut Frame, area: Rect, app: &App) {
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
            value: format!("{:+.0} dB", eq.low_db()),
            bar_pct: ((eq.low_db() + 12.0) / 24.0) as f64,
        },
        Row {
            label: "EQ Mid",
            value: format!("{:+.0} dB", eq.mid_db()),
            bar_pct: ((eq.mid_db() + 12.0) / 24.0) as f64,
        },
        Row {
            label: "EQ High",
            value: format!("{:+.0} dB", eq.high_db()),
            bar_pct: ((eq.high_db() + 12.0) / 24.0) as f64,
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

pub fn render_lastfm(f: &mut Frame, area: Rect, app: &App) {
    let w = 80.min(area.width.saturating_sub(2));
    let h = 24.min(area.height.saturating_sub(2));
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

pub fn render_stats(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.stats_snapshot;

    let w = 80.min(area.width.saturating_sub(2));
    let h = 24.min(area.height.saturating_sub(2));
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

pub fn render_tag_editor(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 60.min(area.width.saturating_sub(2));
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

pub fn render_radio_custom_modal(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 64.min(area.width.saturating_sub(2));
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

pub fn render_radio_browser(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 84.min(area.width.saturating_sub(2));
    let h = 25.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let title =
        " 📻 Radio-Browser Hub (+45k Rádios) — ←/→ Gênero · Enter Tocar · a Fila · f Fav · + Add · Esc Fechar ";
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(title, theme.accent()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Category Carousel bar
            Constraint::Length(2), // Search bar
            Constraint::Min(4),    // Station list
        ])
        .split(inner);

    let accent = parse_color(&theme.colors.accent);
    let primary = parse_color(&theme.colors.primary);
    let secondary = parse_color(&theme.colors.secondary);
    let fg = parse_color(&theme.colors.foreground);
    let muted = parse_color(&theme.colors.muted);

    // Active Category Tabs
    let active_cat_idx = app
        .radio_category_idx
        .min(crate::radio_browser::RadioCategory::ALL.len() - 1);

    let mut cat_spans = Vec::new();
    cat_spans.push(Span::raw(" "));
    for (i, cat) in crate::radio_browser::RadioCategory::ALL.iter().enumerate() {
        let is_selected = i == active_cat_idx;
        if is_selected {
            cat_spans.push(Span::styled(
                format!("[ {} ] ", cat.label()),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
        } else {
            cat_spans.push(Span::styled(
                format!("{} ", cat.label()),
                Style::default().fg(muted),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(cat_spans)), chunks[0]);

    // Search Bar
    let cursor = if app.radio_search_editing { "█" } else { "" };
    let search_text = if app.radio_search_query.is_empty() && !app.radio_search_editing {
        " 🔍 Busca: (digite diretamente ou pressione / para pesquisar +45.000 rádios)".to_string()
    } else {
        format!(" 🔍 Busca: {}{cursor}", app.radio_search_query)
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

    // Station list
    let stations = app.radio_filtered_stations();

    let list_area = chunks[2];

    if stations.is_empty() {
        let empty_msg = if app.radio_search_rx.is_some() {
            "  ⏳ Conectando aos servidores do Radio-Browser e carregando estações…"
        } else {
            "  Nenhuma estação encontrada. Digite um termo ou use ← / → para navegar entre os gêneros."
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
        let codec = st.codec.as_deref().unwrap_or("MP3");
        let bitrate_str = st
            .bitrate
            .map(|b| format!("{b}k"))
            .unwrap_or_else(|| "128k".into());
        let tags_trimmed = if st.tags.len() > 22 {
            format!("{}…", &st.tags[..21])
        } else {
            st.tags.clone()
        };

        let row_line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(accent)),
            Span::styled(fav_icon, Style::default().fg(accent)),
            Span::styled(format!("{:<26} ", st.name), name_style),
            Span::styled(format!(" {:<10} ", country), Style::default().fg(secondary)),
            Span::styled(
                format!(" {:<8} ", format!("{bitrate_str} {codec}")),
                Style::default().fg(primary),
            ),
            Span::styled(format!(" {}", tags_trimmed), Style::default().fg(muted)),
        ]);
        lines.push(row_line);
    }

    f.render_widget(Paragraph::new(lines), list_area);
}

pub fn render_lyrics_modal(f: &mut Frame, area: Rect, app: &mut App) {
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

pub fn render_command_palette(f: &mut Frame, area: Rect, app: &mut App) {
    let w = 78.min(area.width.saturating_sub(2));
    let h = 18.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 4,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let secondary = parse_color(&app.theme.colors.secondary);
    let accent = parse_color(&app.theme.colors.accent);
    let muted = parse_color(&app.theme.colors.muted);
    let fg = parse_color(&app.theme.colors.foreground);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(true))
        .title(Span::styled(
            " 🔍 Command Palette [Ctrl+P / Esc: fechar · Enter: executar · ↑↓: navegar] ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Input prompt line
            Constraint::Length(1), // Separator line
            Constraint::Min(4),    // Matches list
            Constraint::Length(1), // Footer count / hint
        ])
        .split(inner);

    // 1. Input prompt line
    let prompt_icon = if app.command_palette_input.starts_with('>')
        || app.command_palette_input.starts_with(':')
    {
        Span::styled(
            " > ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" 🔍 ", Style::default().fg(secondary))
    };
    let input_text = Span::styled(
        &app.command_palette_input,
        Style::default().fg(fg).add_modifier(Modifier::BOLD),
    );
    let cursor = Span::styled("█", Style::default().fg(accent));
    let placeholder = if app.command_palette_input.is_empty() {
        Span::styled(
            " Digite para buscar comandos, músicas, temas ou '>' para ações…",
            Style::default().fg(muted),
        )
    } else {
        Span::raw("")
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            prompt_icon,
            input_text,
            cursor,
            placeholder,
        ])),
        chunks[0],
    );

    // 2. Separator
    let sep = "─".repeat(chunks[1].width as usize);
    f.render_widget(
        Paragraph::new(Span::styled(sep, Style::default().fg(muted))),
        chunks[1],
    );

    // 3. Matches list
    let list_area = chunks[2];
    let max_visible = list_area.height as usize;
    let total_matches = app.command_palette_matches.len();

    if total_matches == 0 {
        let empty_msg = vec![
            Line::from(""),
            Line::from(Span::styled(
                "   Nenhum resultado correspondente para a busca.",
                Style::default().fg(muted),
            )),
        ];
        f.render_widget(Paragraph::new(empty_msg), list_area);
    } else {
        let selected = app.command_palette_row.min(total_matches.saturating_sub(1));
        let scroll_offset = if selected >= max_visible {
            selected - max_visible + 1
        } else {
            0
        };

        let mut lines = Vec::new();
        for (i, item) in app
            .command_palette_matches
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_visible)
        {
            let is_sel = i == selected;
            let icon = item.category.icon();
            let cat_label = item.category.label();

            let (prefix, title_style, desc_style) = if is_sel {
                (
                    " ▶ ",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    Style::default().fg(fg),
                )
            } else {
                ("   ", Style::default().fg(fg), Style::default().fg(muted))
            };

            let cat_span = Span::styled(
                format!(" [{icon} {cat_label}] "),
                Style::default().fg(if is_sel { secondary } else { muted }),
            );
            let title_span = Span::styled(format!("{:<30} ", item.title), title_style);
            let desc_span = Span::styled(&item.description, desc_style);

            let row_line = Line::from(vec![
                Span::styled(prefix, Style::default().fg(accent)),
                cat_span,
                title_span,
                desc_span,
            ]);

            lines.push(row_line);
        }
        f.render_widget(Paragraph::new(lines), list_area);
    }

    // 4. Footer
    let footer_text = format!(
        " {}/{} resultados ",
        if total_matches > 0 {
            app.command_palette_row + 1
        } else {
            0
        },
        total_matches
    );
    f.render_widget(
        Paragraph::new(Span::styled(footer_text, Style::default().fg(muted))),
        chunks[3],
    );
}

pub fn render_share_modal(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 70.min(area.width.saturating_sub(2));
    let h = 18.min(area.height.saturating_sub(2));
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(" Compartilhar Playlist (Enter: Publicar | Tab: Campo | Esc: Cancelar) ");
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Description
            Constraint::Length(3), // Visibility
            Constraint::Length(3), // Tags
            Constraint::Min(0),
        ])
        .split(inner);

    // 1. Title
    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.share_modal_field == 0 {
            Style::default().fg(accent)
        } else {
            Style::default().fg(muted)
        })
        .title(" Título da Playlist ");
    let title_text = if app.share_playlist_title.is_empty() && app.share_modal_field != 0 {
        Span::styled(
            " (Sem título - usará nome da fila) ",
            Style::default().fg(muted),
        )
    } else {
        Span::styled(
            format!(
                " {}",
                if app.share_modal_field == 0 {
                    format!("{}█", app.share_playlist_title)
                } else {
                    app.share_playlist_title.clone()
                }
            ),
            Style::default().fg(fg),
        )
    };
    f.render_widget(Paragraph::new(title_text).block(title_block), chunks[0]);

    // 2. Description
    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.share_modal_field == 1 {
            Style::default().fg(accent)
        } else {
            Style::default().fg(muted)
        })
        .title(" Descrição (opcional) ");
    let desc_text = Span::styled(
        format!(
            " {}",
            if app.share_modal_field == 1 {
                format!("{}█", app.share_playlist_desc)
            } else {
                app.share_playlist_desc.clone()
            }
        ),
        Style::default().fg(fg),
    );
    f.render_widget(Paragraph::new(desc_text).block(desc_block), chunks[1]);

    // 3. Visibility Toggle
    let vis_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.share_modal_field == 2 {
            Style::default().fg(accent)
        } else {
            Style::default().fg(muted)
        })
        .title(" Visibilidade (Espaço para alternar) ");
    let (vis_label, vis_icon) = match app.share_playlist_visibility {
        crate::share::Visibility::Public => ("Pública (visível no catálogo de descoberta)", "🌍"),
        crate::share::Visibility::Unlisted => ("Não Listada (apenas quem tem o link/ID)", "🔗"),
        crate::share::Visibility::Private => ("Privada (somente você)", "🔒"),
    };
    let vis_text = Span::styled(
        format!(" {vis_icon} {vis_label}"),
        Style::default()
            .fg(if app.share_modal_field == 2 {
                secondary
            } else {
                fg
            })
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(Paragraph::new(vis_text).block(vis_block), chunks[2]);

    // 4. Tags
    let tags_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.share_modal_field == 3 {
            Style::default().fg(accent)
        } else {
            Style::default().fg(muted)
        })
        .title(" Tags (separadas por vírgula) ");
    let tags_text = Span::styled(
        format!(
            " {}",
            if app.share_modal_field == 3 {
                format!("{}█", app.share_playlist_tags)
            } else {
                app.share_playlist_tags.clone()
            }
        ),
        Style::default().fg(fg),
    );
    f.render_widget(Paragraph::new(tags_text).block(tags_block), chunks[3]);
}

pub fn render_browse_modal(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let w = 82.min(area.width.saturating_sub(2));
    let h = 22.min(area.height.saturating_sub(2));
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

    let hint = Line::from(vec![
        Span::styled(" /", Style::default().fg(accent)),
        Span::styled(" buscar  ", Style::default().fg(muted)),
        Span::styled("Enter / i", Style::default().fg(accent)),
        Span::styled(" importar para fila  ", Style::default().fg(muted)),
        Span::styled("Esc", Style::default().fg(accent)),
        Span::styled(" fechar", Style::default().fg(muted)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " 🌐 Descoberta de Playlists Públicas ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(hint);
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Search bar
            Constraint::Min(1),    // Results list
        ])
        .split(inner);

    // Search bar
    let query_display = if app.browse_search_editing {
        format!(" 🔍 Buscar playlists: {}█", app.browse_search_query)
    } else if app.browse_search_query.is_empty() {
        " Pressione / para pesquisar playlists da comunidade…".to_string()
    } else {
        format!(" 🔍 Buscar playlists: {}", app.browse_search_query)
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            query_display,
            Style::default().fg(if app.browse_search_editing { fg } else { muted }),
        )),
        chunks[0],
    );

    // Results
    let list_area = chunks[1];
    let results = &app.browse_results;
    if results.is_empty() {
        let msg = "  Nenhuma playlist encontrada. Digite um termo de busca e tecle Enter.";
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(muted))),
            list_area,
        );
    } else {
        let items: Vec<ListItem> = results
            .iter()
            .enumerate()
            .map(|(i, pl)| {
                let selected = i == app.browse_row;
                let tags_str = if pl.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", pl.tags.join(", "))
                };
                let label = format!(
                    " {} {:<32} by {:<16} ({} faixas) ♥ {}{}",
                    if selected { "▶" } else { " " },
                    pl.name.chars().take(30).collect::<String>(),
                    pl.author.display_name.chars().take(14).collect::<String>(),
                    pl.track_count,
                    pl.likes,
                    tags_str,
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
        state.select(Some(app.browse_row));
        f.render_stateful_widget(
            List::new(items).highlight_style(Style::default().bg(secondary).fg(bg)),
            list_area,
            &mut state,
        );
    }
}
