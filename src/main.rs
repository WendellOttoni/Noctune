use anyhow::Result;

mod app;
mod audio;
mod config;
mod theme;
mod tui;
mod ui;
mod visualizer;

fn main() -> Result<()> {
    let config = config::Config::load_or_default()?;
    let theme = theme::Theme::load(&config.theme)?;

    let mut terminal = tui::init()?;
    let result = app::App::new(config, theme)?.run(&mut terminal);
    tui::restore()?;

    result
}
