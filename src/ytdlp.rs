use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{path::PathBuf, process::{Command, Stdio}, time::Duration};

use crate::audio::{SymphoniaSource, Track};

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

/// Spawn yt-dlp with `-o -` and build a `SymphoniaSource` from its piped stdout.
///
/// With ffmpeg available, yt-dlp remuxes DASH segments into a streaming-friendly
/// Matroska/WebM container before piping — no temp file required and no panic risk.
/// Without ffmpeg, falls back to progressive format 18/22 (non-DASH mp4).
/// The spawned process is killed automatically when the source is dropped.
pub fn spawn_yt_dlp(youtube_url: &str) -> Result<SymphoniaSource> {
    let format_selector = if ffmpeg_available() {
        "bestaudio[ext=webm]/bestaudio[ext=opus]/bestaudio[ext=ogg]/bestaudio[ext=m4a]/bestaudio"
    } else {
        "18/22/best[ext=mp4][protocol^=https]"
    };

    let child = Command::new("yt-dlp")
        .args([
            "-f", format_selector,
            "--no-playlist",
            "--no-warnings",
            "-o", "-",
            youtube_url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| anyhow!("yt-dlp not found — install it: pip install yt-dlp"))?;

    SymphoniaSource::from_child(child, symphonia::core::probe::Hint::new())
        .map_err(|e| anyhow!("yt-dlp stream: {e}"))
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
        replaygain_track_db: None,
        replaygain_album_db: None,
    })
}
