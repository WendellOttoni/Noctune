use anyhow::Result;

mod album_art;
mod app;
mod audio;
mod cache;
mod compressor;
mod config;
mod discord;
mod eq;
mod history;
mod keybinds;
mod lastfm;
mod logging;
mod lyrics;
mod media_session;
mod metadata;
mod plugin;
mod radio;
mod radio_browser;
mod radio_mode;
mod ratings;
mod secrets;
mod share;
mod single_instance;
mod spotify;
mod stats;
mod subsonic;
mod theme;
mod tui;
mod ui;
mod updater;
mod visualizer;
mod ytdlp;

fn main() -> Result<()> {
    let log_opts = logging::parse_cli_flags();
    let _log_guard = logging::init(&log_opts)?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "noctune starting");

    let _instance_guard = match single_instance::SingleInstanceGuard::acquire()? {
        Some(g) => g,
        None => {
            eprintln!("noctune is already running. Exiting.");
            tracing::warn!("another noctune instance is already running; exiting");
            return Ok(());
        }
    };

    let (config, config_warnings) = config::Config::load_or_default()?;
    for w in &config_warnings {
        tracing::warn!(target: "config", "{w}");
        eprintln!("noctune: {w}");
    }
    let theme = theme::Theme::load(&config.theme)?;

    // Initialize art picker before raw mode so terminal queries (Kitty/Sixel/iTerm2
    // cell-size detection) can read from stdio without conflicting with the event loop.
    let art_picker = album_art::ArtPicker::new();

    let mut terminal = tui::init()?;
    let result = app::App::new(config, theme, art_picker)?.run(&mut terminal);
    tui::restore()?;

    result
}
