use anyhow::Result;

mod album_art;
mod app;
mod audio;
mod audio_cache;
mod cache;
mod commands;
mod compressor;
mod config;
mod db;
mod diagnostics;
mod discord;
mod downloader;
mod eq;
mod history;
mod i18n;
mod ipc;
mod keybinds;
mod lastfm;
mod logging;
mod lyrics;
mod media_session;
mod metadata;
mod plugin;
mod process;
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
#[cfg(test)]
mod test_audio;
mod theme;
mod tui;
mod ui;
mod updater;
mod vault;
mod visualizer;
mod worker;
mod ytdlp;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let first = args[1].as_str();
        match first {
            "doctor" => {
                return diagnostics::doctor(
                    args.iter().any(|a| a == "--json"),
                    args.iter().any(|a| a == "--test-audio"),
                )
            }
            "commands" => return diagnostics::commands(),
            "setup" => {
                let music = args
                    .iter()
                    .position(|a| a == "--music")
                    .map(|i| {
                        args.get(i + 1)
                            .map(String::as_str)
                            .ok_or_else(|| anyhow::anyhow!("--music requires a folder"))
                    })
                    .transpose()?;
                diagnostics::setup(music)?;
                return Ok(());
            }
            "--version" | "-V" => {
                println!("noctune {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "play" | "pause" | "toggle" | "play-pause" | "next" | "prev" | "previous" | "stop"
            | "status" | "status-json" => {
                let cmd = if first == "status-json" {
                    "status --json"
                } else {
                    first
                };
                match ipc::IpcClient::send_command(cmd) {
                    Ok(resp) => {
                        println!("{resp}");
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
            }
            "volume" => {
                let arg = if args.len() > 2 {
                    format!("volume {}", args[2])
                } else {
                    "volume".to_string()
                };
                match ipc::IpcClient::send_command(&arg) {
                    Ok(resp) => {
                        println!("{resp}");
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
            }
            "--help" | "-h" => {
                println!(
                    "Noctune — Modern Terminal Music Player (v{})",
                    env!("CARGO_PKG_VERSION")
                );
                println!("\nUsage:");
                println!("  noctune setup [--music <folder>]  Configure a music folder");
                println!("  noctune doctor [--json] [--test-audio]  Diagnose setup");
                println!("  noctune commands         Export current shortcuts as Markdown");
                println!("  noctune                  Launch interactive TUI player");
                println!("  noctune play             Resume playback");
                println!("  noctune pause            Pause playback");
                println!("  noctune toggle           Toggle play / pause");
                println!("  noctune next             Skip to next track");
                println!("  noctune prev             Skip to previous track");
                println!("  noctune stop             Stop playback");
                println!("  noctune volume [val]     Get / adjust volume (e.g. +10, -10, 80)");
                println!("  noctune status           Show currently playing track info");
                println!(
                    "  noctune status --json    Show status formatted as JSON for polybar/waybar"
                );
                println!("  noctune --help           Show this help");
                return Ok(());
            }
            _ => {}
        }
    }

    let first_run = diagnostics::first_run()?;
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
    let mut theme = theme::Theme::load(&config.theme)?;
    if config.ui.simple_symbols {
        theme.use_simple_symbols();
    }

    // Initialize art picker before raw mode so terminal queries (Kitty/Sixel/iTerm2
    // cell-size detection) can read from stdio without conflicting with the event loop.
    let art_picker = album_art::ArtPicker::new();

    let mut app = app::App::new(config, theme, art_picker)?;
    app.first_run_autoplay = first_run;
    let mut terminal = tui::init()?;
    app.run(&mut terminal)
}
