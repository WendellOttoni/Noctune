mod help;
mod modals;
mod panes;
pub(crate) mod util;
mod visualizer;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::app::App;

use self::help::render_help;
use self::modals::*;
use self::panes::*;
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
    if app.show_subsonic_browser {
        render_subsonic_browser(f, area, app);
    }
    if app.show_vault_browser {
        render_vault_browser(f, area, app);
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
    if app.show_command_palette {
        render_command_palette(f, area, app);
    }
}
