use anyhow::Result;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// CLI flags controlling logging behaviour. Parsed by `parse_cli_flags`.
#[derive(Debug, Default)]
pub struct LogOptions {
    /// `--debug` — elevates default filter from WARN to DEBUG.
    pub debug: bool,
    /// `--log-stderr` — write to stderr instead of the rotating file.
    pub stderr: bool,
}

/// Parse a minimal set of CLI flags. Anything else is ignored so we don't have to
/// pull in clap just for two flags.
pub fn parse_cli_flags() -> LogOptions {
    let mut o = LogOptions::default();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--debug" => o.debug = true,
            "--log-stderr" => o.stderr = true,
            _ => {}
        }
    }
    o
}

/// Initialise the tracing subscriber. The returned `WorkerGuard` must be kept
/// alive for the duration of the program — dropping it flushes the appender.
///
/// Filter resolution order:
/// 1. `RUST_LOG` if set (full env-filter syntax).
/// 2. `DEBUG` if `--debug`.
/// 3. `WARN` by default.
pub fn init(opts: &LogOptions) -> Result<LoggingGuard> {
    let default_level = if opts.debug { "debug" } else { "warn" };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("noctune={default_level},warn")));

    if opts.stderr {
        let (nb, guard) = tracing_appender::non_blocking(std::io::stderr());
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(nb).with_ansi(false))
            .init();
        return Ok(LoggingGuard {
            _guard: guard,
            log_path: None,
        });
    }

    let dir = log_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        // Fall back to stderr if we can't write the log dir — keep noise low.
        eprintln!(
            "noctune: log dir {} not writable ({e}); logging to stderr",
            dir.display()
        );
        let (nb, guard) = tracing_appender::non_blocking(std::io::stderr());
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(nb).with_ansi(false))
            .init();
        return Ok(LoggingGuard {
            _guard: guard,
            log_path: None,
        });
    }

    let appender = tracing_appender::rolling::daily(&dir, "noctune.log");
    let (nb, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(nb).with_ansi(false))
        .init();

    Ok(LoggingGuard {
        _guard: guard,
        log_path: Some(dir.join("noctune.log")),
    })
}

pub struct LoggingGuard {
    _guard: WorkerGuard,
    pub log_path: Option<PathBuf>,
}

fn log_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "noctune", "noctune")
        .map(|p| p.data_local_dir().join("logs"))
        .unwrap_or_else(|| PathBuf::from("noctune-logs"))
}
