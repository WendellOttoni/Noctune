use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{path::PathBuf, process::{Command, Stdio}, sync::{Arc, Mutex}, time::Duration};

use crate::audio::{SymphoniaSource, Track};

fn yt_dlp_install_hint() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp not found — install it with: winget install yt-dlp.yt-dlp  (then restart your terminal)"
    } else {
        "yt-dlp not found — install it with: pip install yt-dlp"
    }
}

pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/")
        || url.contains("youtu.be/")
        || url.contains("music.youtube.com/")
        || url.starts_with("ytsearch:")
        || url.starts_with("ytmsearch:")
        || url.starts_with("ytsearch5:")
}

#[allow(dead_code)]
pub fn check_available() -> bool {
    Command::new("yt-dlp")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Download audio and build a `SymphoniaSource`.
///
/// On Unix: spawns yt-dlp with `-o -` and pipes stdout directly — no temp file.
/// On Windows: pipes are unreliable for binary audio output (EINVAL), so yt-dlp
/// writes to a temp file which is read and deleted immediately after.
pub fn spawn_yt_dlp(youtube_url: &str, stream_err: Arc<Mutex<Option<String>>>) -> Result<SymphoniaSource> {
    spawn_yt_dlp_at(youtube_url, Duration::ZERO, stream_err)
}

/// Same as `spawn_yt_dlp` but starts playback at `start_offset`. Used by seek in
/// streams (#57). Requires ffmpeg for non-zero offsets; falls back to a regular
/// (start-from-zero) spawn when ffmpeg is missing.
pub fn spawn_yt_dlp_at(
    youtube_url: &str,
    start_offset: Duration,
    stream_err: Arc<Mutex<Option<String>>>,
) -> Result<SymphoniaSource> {
    let offset_secs = start_offset.as_secs();
    let want_offset = offset_secs > 0 && ffmpeg_available();
    if cfg!(target_os = "windows") {
        download_via_tempfile(youtube_url, want_offset.then_some(offset_secs), stream_err)
    } else {
        pipe_from_yt_dlp(youtube_url, want_offset.then_some(offset_secs), stream_err)
    }
}

fn pipe_from_yt_dlp(
    youtube_url: &str,
    start_secs: Option<u64>,
    stream_err: Arc<Mutex<Option<String>>>,
) -> Result<SymphoniaSource> {
    let format_selector = if ffmpeg_available() {
        "bestaudio[ext=webm]/bestaudio[ext=opus]/bestaudio[ext=ogg]/bestaudio[ext=m4a]/bestaudio"
    } else {
        "18/22/best[ext=mp4][protocol^=https]"
    };

    let mut args: Vec<String> = vec![
        "-f".into(), format_selector.into(),
        "--no-playlist".into(), "--no-warnings".into(),
        "-o".into(), "-".into(),
    ];
    if let Some(secs) = start_secs {
        // --download-sections asks yt-dlp to pipe via ffmpeg with `-ss`, so the stream
        // already starts at the requested offset and seeking is instant on the player
        // side (no per-sample skipping, no re-download from zero).
        args.push("--download-sections".into());
        args.push(format!("*{secs}-inf"));
        args.push("--force-keyframes-at-cuts".into());
    }
    args.push(youtube_url.into());

    let child = Command::new("yt-dlp")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("{}\n(internal: {e})", yt_dlp_install_hint()))?;

    SymphoniaSource::from_child(child, symphonia::core::probe::Hint::new(), stream_err)
        .map_err(|e| anyhow!("yt-dlp stream: {e}"))
}

fn download_via_tempfile(
    youtube_url: &str,
    start_secs: Option<u64>,
    _stream_err: Arc<Mutex<Option<String>>>,
) -> Result<SymphoniaSource> {
    // M4A (AAC 128kbps) is the best format symphonia reliably decodes from YouTube.
    // DASH WebM/Opus has higher bitrate but triggers "unsupported feature: core" in
    // symphonia's Matroska reader when YouTube serves it as fragmented DASH segments.
    let format_selector =
        "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio[ext=ogg]/18/bestaudio";

    let tmp_dir = std::env::temp_dir();
    let hash: u64 = youtube_url
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let tmp_pattern = tmp_dir.join(format!("noctune_{hash}.%(ext)s")).to_string_lossy().to_string();
    let tmp_base = tmp_dir.join(format!("noctune_{hash}"));

    let mut args: Vec<String> = vec![
        "-f".into(), format_selector.into(),
        "--no-playlist".into(), "--no-warnings".into(),
        "-o".into(), tmp_pattern.clone(),
    ];
    if let Some(secs) = start_secs {
        args.push("--download-sections".into());
        args.push(format!("*{secs}-inf"));
        args.push("--force-keyframes-at-cuts".into());
    }
    args.push(youtube_url.into());

    let out = Command::new("yt-dlp")
        .args(&args)
        .output()
        .map_err(|e| anyhow!("{}\n(internal: {e})", yt_dlp_install_hint()))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("yt-dlp: {}", err.trim()));
    }

    for ext in &["mp4", "m4a", "webm", "opus", "ogg", "mp3", "aac", "wav"] {
        let path = tmp_base.with_extension(ext);
        if path.exists() {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let _ = std::fs::remove_file(&path);
            let hint = symphonia::core::probe::Hint::new();
            return SymphoniaSource::from_bytes(bytes, hint)
                .map_err(|e| anyhow!("decode: {e}"));
        }
    }

    Err(anyhow!("yt-dlp: could not locate downloaded audio file"))
}

#[derive(Debug, Deserialize)]
struct YtInfo {
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    webpage_url: Option<String>,
    // flat-playlist entries use "url" for the watch page
    url: Option<String>,
    id: Option<String>,
    entries: Option<Vec<YtInfo>>,
    #[serde(rename = "_type")]
    #[allow(dead_code)]
    entry_type: Option<String>,
}

/// Fetch track metadata for a URL (single video, playlist, or search query).
/// Stores the watch-page URL in `path`; stream URL is resolved at play time.
pub fn fetch_tracks(url: &str) -> Result<Vec<Track>> {
    // Expand bare ytsearch: to top 5 results
    let resolved = if url.starts_with("ytsearch:") && !url[9..].starts_with(|c: char| c.is_ascii_digit()) {
        format!("ytsearch5:{}", &url[9..])
    } else {
        url.to_string()
    };

    let output = Command::new("yt-dlp")
        .args([
            "-J",
            "--flat-playlist",
            "--no-warnings",
            &resolved,
        ])
        .output()
        .map_err(|_| anyhow!("{}", yt_dlp_install_hint()))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("yt-dlp: {}", err.trim()));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let info: YtInfo = serde_json::from_str(&json_str)
        .map_err(|e| anyhow!("yt-dlp JSON parse error: {e}"))?;

    if let Some(entries) = info.entries {
        // Playlist / channel
        let tracks = entries.into_iter().filter_map(yt_info_to_track).collect();
        Ok(tracks)
    } else {
        // Single video
        Ok(yt_info_to_track(info).into_iter().collect())
    }
}

fn yt_info_to_track(info: YtInfo) -> Option<Track> {
    // webpage_url preferred; flat-playlist entries use "url"; fallback: construct from id
    let watch_url = info
        .webpage_url
        .or(info.url)
        .or_else(|| info.id.as_ref().map(|id| format!("https://www.youtube.com/watch?v={id}")))?;

    let title = info.title.unwrap_or_else(|| "Unknown".to_string());
    let artist = info.uploader.or(info.channel);
    let duration = info.duration.map(Duration::from_secs_f64);

    Some(Track {
        path: PathBuf::from(watch_url),
        title,
        artist,
        album: None,
        genre: None,
        year: None,
        duration,
        replaygain_track_db: None,
        replaygain_album_db: None,
    })
}
