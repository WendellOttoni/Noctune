use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{path::PathBuf, process::Command, time::Duration};

use crate::audio::Track;

pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/") || url.contains("youtu.be/") || url.contains("music.youtube.com/")
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

/// Download audio for a YouTube URL to a temp file, read it, then delete it.
///
/// Piping with `-o -` delivers raw DASH segments (no seek table), causing
/// symphonia to panic during init. Writing to a file lets yt-dlp merge the
/// DASH segments into a proper seekable container before we read the bytes.
pub fn download_audio_bytes(youtube_url: &str) -> Result<Vec<u8>> {
    let tmp_dir = std::env::temp_dir();
    let hash: u64 = youtube_url
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    // %(ext)s is expanded by yt-dlp to the actual extension
    let tmp_pattern = tmp_dir
        .join(format!("noctune_{hash}.%(ext)s"))
        .to_string_lossy()
        .to_string();
    let tmp_base = tmp_dir.join(format!("noctune_{hash}"));

    // YouTube serves audio-only formats as DASH (fragmented MP4 / fragmented WebM),
    // which symphonia's MP4 reader can't seek — rodio's init then panics.
    // Without ffmpeg, yt-dlp cannot remux DASH into a regular container.
    // Workaround: pick a *progressive* (non-DASH) format that includes video+audio
    // in a properly indexed mp4 container — symphonia can still extract the audio track.
    let format_selector = if ffmpeg_available() {
        // With ffmpeg, yt-dlp auto-remuxes DASH m4a into proper mp4
        "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio"
    } else {
        // No ffmpeg: prefer progressive (non-DASH) mp4 formats (18 = 360p+aac, 22 = 720p+aac)
        "18/22/best[ext=mp4][protocol^=https]"
    };

    let out = Command::new("yt-dlp")
        .args([
            "-f", format_selector,
            "--no-playlist",
            "--no-warnings",
            "-o", &tmp_pattern,
            youtube_url,
        ])
        .output()
        .map_err(|_| anyhow!("yt-dlp not found — install it: pip install yt-dlp"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("yt-dlp: {}", err.trim()));
    }

    // yt-dlp expanded %(ext)s — find the actual file
    for ext in &["mp4", "m4a", "webm", "opus", "ogg", "mp3", "aac", "wav"] {
        let path = tmp_base.with_extension(ext);
        if path.exists() {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let _ = std::fs::remove_file(&path);
            return Ok(bytes);
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

/// Fetch track metadata for a URL (single video or playlist).
/// Stores the watch-page URL in `path`; stream URL is resolved at play time.
pub fn fetch_tracks(url: &str) -> Result<Vec<Track>> {
    let output = Command::new("yt-dlp")
        .args([
            "-J",                // full JSON dump
            "--flat-playlist",   // don't resolve individual stream URLs (fast)
            "--no-warnings",
            url,
        ])
        .output()
        .map_err(|_| anyhow!("yt-dlp not found — install it: pip install yt-dlp"))?;

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
    })
}
