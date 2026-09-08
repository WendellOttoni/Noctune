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
    let lang = i18n::Language::configured();
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
                let music =
                    args.iter()
                        .position(|a| a == "--music")
                        .map(|i| {
                            args.get(i + 1).map(String::as_str).ok_or_else(|| {
                                anyhow::anyhow!(lang
                                    .text("--music requires a folder", "--music exige uma pasta"))
                            })
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
                    "{}",
                    crate::localized_format!(
                        lang,
                        "Noctune — Modern Terminal Music Player (v{})",
                        "Noctune — Reprodutor de Música para Terminal (v{})",
                        env!("CARGO_PKG_VERSION")
                    )
                );
                println!("{}", lang.text("\nUsage:", "\nUso:"));
                println!(
                    "{}",
                    lang.text(
                        "  noctune setup [--music <folder>]  Configure a music folder",
                        "  noctune setup [--music <folder>]  Configurar uma pasta de músicas"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune doctor [--json] [--test-audio]  Diagnose setup",
                        "  noctune doctor [--json] [--test-audio]  Diagnosticar a configuração"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune commands         Export current shortcuts as Markdown",
                        "  noctune commands         Exportar atalhos atuais em Markdown"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune                  Launch interactive TUI player",
                        "  noctune                  Abrir o reprodutor interativo"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune play             Resume playback",
                        "  noctune play             Retomar reprodução"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune pause            Pause playback",
                        "  noctune pause            Pausar reprodução"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune toggle           Toggle play / pause",
                        "  noctune toggle           Alternar reprodução / pausa"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune next             Skip to next track",
                        "  noctune next             Ir para a próxima faixa"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune prev             Skip to previous track",
                        "  noctune prev             Ir para a faixa anterior"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune stop             Stop playback",
                        "  noctune stop             Parar reprodução"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune volume [val]     Get / adjust volume (e.g. +10, -10, 80)",
                        "  noctune volume [val]     Consultar / ajustar volume (ex.: +10, -10, 80)"
                    )
                );
                println!(
                    "{}",
                    lang.text(
                        "  noctune status           Show currently playing track info",
                        "  noctune status           Mostrar informações da faixa atual"
                    )
                );
                println!("{}", lang.text("  noctune status --json    Show status formatted as JSON for polybar/waybar", "  noctune status --json    Mostrar estado em JSON para polybar/waybar"));
                println!(
                    "{}",
                    lang.text(
                        "  noctune --help           Show this help",
                        "  noctune --help           Mostrar esta ajuda"
                    )
                );
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
            eprintln!(
                "{}",
                lang.text(
                    "noctune is already running. Exiting.",
                    "O noctune já está em execução. Encerrando."
                )
            );
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
