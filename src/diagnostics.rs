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
    let lang = config.ui.language;
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
        println!(
            "{}",
            crate::localized_format!(
                lang,
                "Noctune {} — diagnostics",
                "Noctune {} — diagnóstico",
                env!("CARGO_PKG_VERSION")
            )
        );
        println!("Config: {}", crate::config::config_path()?.display());
        for (key, value) in report.as_object().unwrap() {
            let label = match key.as_str() {
                "version" => lang.text("version", "versão"),
                "os" => lang.text("os", "sistema"),
                "arch" => lang.text("arch", "arquitetura"),
                "music_roots" => lang.text("music_roots", "pastas de músicas"),
                "available_roots" => lang.text("available_roots", "pastas disponíveis"),
                "audio_output_available" => {
                    lang.text("audio_output_available", "saída de áudio disponível")
                }
                "yt_dlp_available" => lang.text("yt_dlp_available", "yt-dlp disponível"),
                "ffmpeg_available" => lang.text("ffmpeg_available", "FFmpeg disponível"),
                "spotify_configured" => lang.text("spotify_configured", "Spotify configurado"),
                "spotify_native_audio" => {
                    lang.text("spotify_native_audio", "áudio nativo do Spotify")
                }
                "subsonic_configured" => lang.text("subsonic_configured", "Subsonic configurado"),
                "native_credentials" => lang.text("native_credentials", "credenciais nativas"),
                _ => key.as_str(),
            };
            let display = match value.as_bool() {
                Some(true) => lang.text("yes", "sim").to_string(),
                Some(false) => lang.text("no", "não").to_string(),
                None => value.to_string(),
            };
            println!("{label}: {display}");
        }
        println!(
            "{}",
            lang.text(
                "Missing audio: select an output device. Missing folders: run noctune setup.",
                "Sem áudio: selecione uma saída. Pastas ausentes: execute noctune setup."
            )
        );
        println!("{}", lang.text("YouTube needs yt-dlp; seeking streams may also need FFmpeg. Restart your terminal after installation.", "O YouTube precisa do yt-dlp; avançar ou retroceder transmissões pode exigir FFmpeg. Reinicie o terminal após instalar."));
        println!("{}", lang.text("Shareable report: noctune doctor --json (no account identifiers or paths).", "Relatório para compartilhar: noctune doctor --json (sem identificadores de contas ou caminhos)."));
    }
    if test_audio {
        use rodio::Source;
        let (_stream, handle) = rodio::OutputStream::try_default().context(lang.text(
            "Cannot open default audio output",
            "Não foi possível abrir a saída de áudio padrão",
        ))?;
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
    let lang = config.ui.language;
    let folder = if let Some(folder) = music {
        folder.to_string()
    } else {
        anyhow::ensure!(
            std::io::stdin().is_terminal(),
            "{}",
            lang.text(
                "Use noctune setup --music <folder> without a terminal",
                "Use noctune setup --music <folder> fora de um terminal"
            )
        );
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
        "{}",
        lang.text(
            "Music folder is unavailable; configuration preserved",
            "Pasta de músicas indisponível; configuração preservada"
        )
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
