//! Help overlay (#100). Extracted from `ui.rs` — pure code move.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::theme::{parse_color, Theme};

pub fn render_help(f: &mut Frame, area: Rect, scroll: u16, theme: &Theme) {
    let w = 62.min(area.width.saturating_sub(2));
    let h = area.height.saturating_sub(2).max(6);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let lines: Vec<Line> = vec![
        Line::from("  Playback"),
        Line::from("    Space        play / pause"),
        Line::from("    n / p        next / previous"),
        Line::from("    s            stop"),
        Line::from("    ← / →        seek -5s / +5s"),
        Line::from("    + / -        volume up / down"),
        Line::from("    m            toggle mini mode"),
        Line::from("    Ctrl+P       open Command Palette (search & actions)"),
        Line::from(""),
        Line::from("  Library / Queue"),
        Line::from("    Tab          switch focus"),
        Line::from("    ↑↓ / jk      move selection"),
        Line::from("    Enter        play selected"),
        Line::from("    a / d        add / remove from queue"),
        Line::from("    c            clear queue (confirm twice)"),
        Line::from("    u            undo last clear / remove"),
        Line::from("    f            toggle favorite"),
        Line::from("    /            search (title, artist, album, genre, year)"),
        Line::from("    H            toggle recently played view"),
        Line::from(""),
        Line::from("  Modes & playlists"),
        Line::from("    S            toggle shuffle"),
        Line::from("    r            cycle repeat (off/all/one)"),
        Line::from("    o            cycle sort (title/artist/album/year)"),
        Line::from("    T            edit track tags / metadata"),
        Line::from("    w            save queue as .m3u / .m3u8"),
        Line::from("    L            load / browse saved playlists (.m3u/.m3u8)"),
        Line::from("    O            open profiles browser (save/load settings)"),
        Line::from(""),
        Line::from("  Quick Views"),
        Line::from("    1            switch to Library view"),
        Line::from("    2            focus Queue"),
        Line::from("    3            open Dedicated Radio Mode"),
        Line::from("    4            open Folders Browser"),
        Line::from(""),
        Line::from("  View & Themes"),
        Line::from("    V            cycle view mode (flat/albums/smart/browser/radio)"),
        Line::from("    Shift+Tab    cycle color themes"),
        Line::from("    Click        play library / queue row"),
        Line::from("    Click bar    seek to position"),
        Line::from("    Scroll       scroll list"),
        Line::from(""),
        Line::from("  EQ & Audio"),
        Line::from("    e            open audio panel (EQ, crossfade, speed, viz sens…)"),
        Line::from(
            "    0 / Shift+E  cycle EQ presets (Flat, Bass Boost, Vocal, Rock, Electronic…)",
        ),
        Line::from("    Ctrl+S       save custom preset (in EQ tuner)"),
        Line::from("    y            open synced lyrics / Karaoke popup"),
        Line::from("    v            cycle viz mode (spectrum/waveform/vu/waterfall/oscilloscope)"),
        Line::from("    [ / ]        viz sensitivity -/+"),
        Line::from("    G            cycle ReplayGain (off/track/album)"),
        Line::from(""),
        Line::from("  Library tools"),
        Line::from("    R            rescan music_dirs"),
        Line::from("    I            track info popup"),
        Line::from("    D            select audio output device"),
        Line::from(""),
        Line::from("  Online / Streaming"),
        Line::from("    3 / K        open Online Radio Mode (+40k stations & curated)"),
        Line::from("    Ctrl+N       open Subsonic / Navidrome Cloud browser"),
        Line::from("    Shift+U      check and apply app update in-place"),
        Line::from("    i            open URL prompt"),
        Line::from("                 • YouTube / youtu.be links"),
        Line::from("                 • SoundCloud links & scsearch:query"),
        Line::from("                 • ytsearch:query  (top 5 results)"),
        Line::from("                 • ytmsearch:query (YouTube Music)"),
        Line::from("                 • M3U / PLS radio playlist URLs"),
        Line::from("                 • Direct HTTP stream URL"),
        Line::from(""),
        Line::from("  Integrations & System"),
        Line::from("    F            Last.fm login (press twice to confirm)"),
        Line::from("    P            Spotify browser login (OAuth PKCE)"),
        Line::from("    @            Spotify play/pause toggle"),
        Line::from("    U            check & apply app update from GitHub"),
    ];

    let p = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(parse_color(&theme.colors.foreground)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border(true))
                .title(Span::styled(
                    " Help  ↑↓ scroll · any other key closes ",
                    theme.accent(),
                )),
        )
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}
