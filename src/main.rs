use anyhow::Result;

mod album_art;
mod stats;
mod app;
mod audio;
mod cache;
mod compressor;
mod config;
mod discord;
mod radio;
mod radio_mode;
mod eq;
mod keybinds;
mod history;
mod lastfm;
mod lyrics;
mod media_session;
mod metadata;
mod ratings;
mod spotify;
mod theme;
mod tui;
mod ui;
mod visualizer;
mod ytdlp;

fn main() -> Result<()> {
    let config = config::Config::load_or_default()?;
    let theme = theme::Theme::load(&config.theme)?;

    // Initialize art picker before raw mode so terminal queries (Kitty/Sixel/iTerm2
    // cell-size detection) can read from stdio without conflicting with the event loop.
    let art_picker = album_art::ArtPicker::new();

    let mut terminal = tui::init()?;
    let result = app::App::new(config, theme, art_picker)?.run(&mut terminal);
    tui::restore()?;

    result
}
