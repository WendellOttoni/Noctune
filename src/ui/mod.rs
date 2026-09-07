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
    if app.config.ui.simple_symbols {
        app.theme.use_simple_symbols();
    }
    let area = f.area();
    if area.width < 40 || area.height < 12 {
        f.render_widget(
            ratatui::widgets::Paragraph::new(app.config.ui.language.text(
                "Terminal too small. Resize to 40x12. q: quit, ?: help",
                "Terminal pequeno. Aumente para 40x12. q: sair, ?: ajuda",
            ))
            .wrap(ratatui::widgets::Wrap { trim: true }),
            area,
        );
        if app.show_help {
            render_help(
                f,
                area,
                app.help_scroll,
                &app.theme,
                &app.bindings,
                app.config.ui.language,
            );
        }
        return;
    }

    if app.mini_mode {
        render_mini(f, area, app);
        if app.show_help {
            render_help(
                f,
                area,
                app.help_scroll,
                &app.theme,
                &app.bindings,
                app.config.ui.language,
            );
        }
        if app.url_editing || app.url_rx.is_some() {
            render_url_input(f, area, app);
        }
        if app.show_command_palette {
            render_command_palette(f, area, app);
        }
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if area.height < 35 { 3 } else { 8 }),
            Constraint::Min(3),
            Constraint::Length(if area.height < 35 { 0 } else { 8 }),
            Constraint::Length(if area.height < 20 { 4 } else { 7 }),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, chunks[0], app);
    if app.show_audio_panel {
        render_audio_panel(f, chunks[1], app);
    } else {
        render_main(f, chunks[1], app);
    }
    if chunks[2].height > 0 {
        render_visualizer(f, chunks[2], app);
    }
    render_now_playing(f, chunks[3], app);
    render_status(f, chunks[4], app);

    if app.show_help {
        render_help(
            f,
            area,
            app.help_scroll,
            &app.theme,
            &app.bindings,
            app.config.ui.language,
        );
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
    if app.show_share_modal {
        render_share_modal(f, area, app);
    }
    if app.show_browse_modal {
        render_browse_modal(f, area, app);
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
    if app.url_editing || app.url_rx.is_some() {
        render_url_input(f, area, app);
    }
}
