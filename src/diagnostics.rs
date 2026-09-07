use anyhow::{Context, Result};
use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn read_config() -> Result<crate::config::Config> {
    let path = crate::config::config_path()?;
    if !path.exists() {
        return Ok(Default::default());
    }
    // Do not include parser excerpts: the TOML may contain credentials.
    toml::from_str(&std::fs::read_to_string(path)?)
        .map_err(|_| anyhow::anyhow!("Invalid config.toml; existing file preserved"))
}

fn available(executable: &str, arg: &str) -> bool {
    let mut command = Command::new(executable);
    command
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            _ => {}
        }
        if started.elapsed() > Duration::from_secs(3) {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// This export deliberately contains no tokens, URLs, usernames, or local paths.
pub fn doctor(json: bool, test_audio: bool) -> Result<()> {
    let config = read_config()?;
    let report = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "music_roots": config.music_dirs.len(), "available_roots": config.music_dirs.iter().filter(|p| p.is_dir()).count(),
        "audio_output_available": crate::audio::default_device_name().is_some(),
        "yt_dlp_available": available(&crate::ytdlp::yt_dlp_executable(), "--version"),
        "ffmpeg_available": available("ffmpeg", "-version"),
        "spotify_configured": config.spotify.is_configured(), "spotify_native_audio": false,
        "subsonic_configured": config.subsonic.is_configured(),
        "native_credentials": cfg!(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Noctune {} — diagnostics", env!("CARGO_PKG_VERSION"));
        println!("Config: {}", crate::config::config_path()?.display());
        for (key, value) in report.as_object().unwrap() {
            println!("{key}: {value}");
        }
        println!("Missing audio: select an output device. Missing folders: run noctune setup.");
        println!("YouTube needs yt-dlp; seeking streams may also need FFmpeg. Restart your terminal after installation.");
        println!("Shareable report: noctune doctor --json (no account identifiers or paths).");
    }
    if test_audio {
        use rodio::Source;
        let (_stream, handle) =
            rodio::OutputStream::try_default().context("Cannot open default audio output")?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.append(
            rodio::source::SineWave::new(440.0)
                .take_duration(Duration::from_millis(300))
                .amplify(0.05),
        );
        sink.sleep_until_end();
    }
    Ok(())
}

pub fn setup(music: Option<&str>) -> Result<bool> {
    let mut config = read_config()?;
    let folder = if let Some(folder) = music {
        folder.to_string()
    } else {
        anyhow::ensure!(
            std::io::stdin().is_terminal(),
            "Use noctune setup --music <folder> without a terminal"
        );
        let lang = config.ui.language;
        println!(
            "{}",
            lang.text(
                "Welcome to Noctune. Choose a music folder to start listening.",
                "Bem-vindo ao Noctune. Escolha uma pasta de músicas para começar."
            )
        );
        print!(
            "{}",
            lang.text(
                "Music folder (Enter skips): ",
                "Pasta de músicas (Enter pula): "
            )
        );
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        input.trim().trim_matches('"').to_string()
    };
    if folder.is_empty() {
        return Ok(false);
    }
    let path = PathBuf::from(folder);
    anyhow::ensure!(
        path.is_dir(),
        "Music folder is unavailable; configuration preserved"
    );
    let path = path.canonicalize()?;
    if !config.music_dirs.contains(&path) {
        config.music_dirs.push(path);
    }
    config.save()?;
    println!(
        "{}",
        config.ui.language.text(
            "Folder saved. Select a track with Enter; D chooses the audio output.",
            "Pasta salva. Enter toca uma faixa; D escolhe a saída de áudio."
        )
    );
    Ok(true)
}

pub fn first_run() -> Result<bool> {
    if !crate::config::config_path()?.exists() && std::io::stdin().is_terminal() {
        setup(None)
    } else {
        Ok(false)
    }
}

pub fn commands() -> Result<()> {
    let config = read_config()?;
    let (bindings, _) = crate::keybinds::Bindings::from_config(&config.keybinds);
    print!(
        "{}",
        crate::commands::markdown(&bindings, config.ui.language)
    );
    Ok(())
}
