use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::audio::{SymphoniaSource, Track};

fn yt_dlp_install_hint() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp not found — install it with: winget install yt-dlp.yt-dlp  (then restart your terminal)"
    } else {
        "yt-dlp not found — install it with: pip install yt-dlp"
    }
}

/// Locate yt-dlp. On Windows the process PATH may differ from the user's shell PATH
/// (e.g. winget-installed binary not yet visible to an already-running launcher), so
/// also probe the standard winget shim directories before giving up (issue #55, bug 3).
pub(crate) fn yt_dlp_executable() -> String {
    if cfg!(target_os = "windows") {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let candidates = [
                PathBuf::from(&local).join("Microsoft\\WinGet\\Links\\yt-dlp.exe"),
                PathBuf::from(&local).join("Microsoft\\WindowsApps\\yt-dlp.exe"),
            ];
            for c in candidates {
                if c.exists() {
                    return c.to_string_lossy().into_owned();
                }
            }
        }
    }
    "yt-dlp".into()
}

/// Classification of yt-dlp / network failures used by the retry logic (#69).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YtErrorKind {
    /// Network blip — retry with normal backoff.
    Transient,
    /// HTTP 429 / 'too many requests' — back off significantly longer.
    RateLimit,
    /// Video removed, geo-blocked, age-gated, etc. — no point retrying.
    Permanent,
}

fn classify(stderr_lower: &str) -> YtErrorKind {
    if stderr_lower.contains("429")
        || stderr_lower.contains("too many requests")
        || stderr_lower.contains("rate limit")
    {
        return YtErrorKind::RateLimit;
    }
    if stderr_lower.contains("private")
        || stderr_lower.contains("removed")
        || stderr_lower.contains("unavailable")
        || stderr_lower.contains("age")
        || stderr_lower.contains("copyright")
        || stderr_lower.contains("not found")
        || stderr_lower.contains("404")
        || stderr_lower.contains("403")
        || stderr_lower.contains("forbidden")
    {
        return YtErrorKind::Permanent;
    }
    YtErrorKind::Transient
}

/// Tunables read from `config.toml` `[ytdlp]` (set by `App::new`).
static RETRY_CFG: std::sync::OnceLock<crate::config::YtdlpConfig> = std::sync::OnceLock::new();

pub fn configure_retries(cfg: crate::config::YtdlpConfig) {
    let _ = RETRY_CFG.set(cfg);
}

fn retry_cfg() -> crate::config::YtdlpConfig {
    RETRY_CFG.get().cloned().unwrap_or_default()
}

/// Run a fallible yt-dlp operation with exponential backoff. `classify_err`
/// inspects the error string and decides whether to retry, give up, or wait
/// longer (for rate limits). Returns the last error if all attempts fail.
fn with_retry<T, F>(mut op: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let cfg = retry_cfg();
    let mut backoff = cfg.backoff_secs.max(1);
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                let msg = format!("{e}").to_lowercase();
                let kind = classify(&msg);
                if kind == YtErrorKind::Permanent || attempt >= cfg.max_retries.max(1) {
                    return Err(e);
                }
                let wait = if kind == YtErrorKind::RateLimit {
                    cfg.ratelimit_backoff_secs.max(backoff)
                } else {
                    backoff
                };
                std::thread::sleep(Duration::from_secs(wait));
                backoff = backoff.saturating_mul(2);
            }
        }
    }
}

fn spawn_error(e: std::io::Error) -> anyhow::Error {
    // Issue #55, bug 2: previously any spawn error was rewritten to the install hint,
    // hiding the real cause (permission denied, AV block, broken winget shim, …).
    // Surface the OS error so the user can diagnose, but keep the install hint for
    // the common case where the binary really is missing.
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow!("{}", yt_dlp_install_hint())
    } else {
        anyhow!("{}\n(internal: {e})", yt_dlp_install_hint())
    }
}

pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/")
        || url.contains("youtu.be/")
        || url.contains("music.youtube.com/")
        || url.contains("soundcloud.com/")
        || url.starts_with("ytsearch:")
        || url.starts_with("ytmsearch:")
        || url.starts_with("ytsearch5:")
        || url.starts_with("scsearch:")
        || url.starts_with("scsearch5:")
}

pub fn check_available() -> bool {
    Command::new(yt_dlp_executable())
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
pub fn spawn_yt_dlp(
    youtube_url: &str,
    stream_err: Arc<Mutex<Option<String>>>,
) -> Result<SymphoniaSource> {
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
    // #69: wrap the spawn in a backoff retry — transient network errors no longer kill
    // playback. Permanent errors (video removed, private) short-circuit immediately.
    with_retry(|| {
        if cfg!(target_os = "windows") {
            download_via_tempfile(
                youtube_url,
                want_offset.then_some(offset_secs),
                stream_err.clone(),
            )
        } else {
            pipe_from_yt_dlp(
                youtube_url,
                want_offset.then_some(offset_secs),
                stream_err.clone(),
            )
        }
    })
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
        "-f".into(),
        format_selector.into(),
        "--no-playlist".into(),
        "--no-warnings".into(),
        "-o".into(),
        "-".into(),
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

    let child = Command::new(yt_dlp_executable())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;

    SymphoniaSource::from_child(child, symphonia::core::probe::Hint::new(), stream_err)
        .map_err(|e| anyhow!("yt-dlp stream: {e}"))
}

fn download_via_tempfile(
    youtube_url: &str,
    start_secs: Option<u64>,
    _stream_err: Arc<Mutex<Option<String>>>,
) -> Result<SymphoniaSource> {
    let cache_dir = crate::config::audio_cache_dir().unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&cache_dir);

    let hash: u64 = youtube_url
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let is_seek = start_secs.is_some_and(|s| s > 0);

    let base_dir = if is_seek {
        std::env::temp_dir()
    } else {
        cache_dir
    };
    let base_path = base_dir.join(format!("noctune_{hash}"));

    // Check if full track is already cached
    if !is_seek {
        for ext in &["mp4", "m4a", "webm", "opus", "ogg", "mp3", "aac", "wav"] {
            let path = base_path.with_extension(ext);
            if path.exists() {
                if let Ok(bytes) = std::fs::read(&path) {
                    if !bytes.is_empty() {
                        let hint = symphonia::core::probe::Hint::new();
                        if let Ok(src) = SymphoniaSource::from_bytes(bytes, hint) {
                            return Ok(src);
                        }
                    }
                }
            }
        }
    }

    // M4A (AAC 128kbps) is the best format symphonia reliably decodes from YouTube.
    let format_selector = "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio[ext=ogg]/18/bestaudio";
    let pattern = base_dir
        .join(format!("noctune_{hash}.%(ext)s"))
        .to_string_lossy()
        .to_string();

    let mut args: Vec<String> = vec![
        "-f".into(),
        format_selector.into(),
        "--no-playlist".into(),
        "--no-warnings".into(),
        "-o".into(),
        pattern,
    ];
    if let Some(secs) = start_secs {
        args.push("--download-sections".into());
        args.push(format!("*{secs}-inf"));
        args.push("--force-keyframes-at-cuts".into());
    }
    args.push(youtube_url.into());

    let out = Command::new(yt_dlp_executable())
        .args(&args)
        .output()
        .map_err(spawn_error)?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err_trim = err.trim();
        if err_trim.contains("403") || err_trim.to_lowercase().contains("forbidden") {
            return Err(anyhow!("yt-dlp: HTTP 403 Forbidden (atualize o yt-dlp com 'yt-dlp -U')"));
        }
        return Err(anyhow!("yt-dlp: {}", err_trim));
    }

    for ext in &["mp4", "m4a", "webm", "opus", "ogg", "mp3", "aac", "wav"] {
        let path = base_path.with_extension(ext);
        if path.exists() {
            let bytes =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            if is_seek {
                let _ = std::fs::remove_file(&path);
            }
            let hint = symphonia::core::probe::Hint::new();
            return SymphoniaSource::from_bytes(bytes, hint).map_err(|e| anyhow!("decode: {e}"));
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
    // #105: album-art source. `thumbnail` is the single "best" URL yt-dlp
    // picked; `thumbnails` is the full list with dimensions. Prefer the list
    // (lets us cap resolution), fall back to the scalar.
    thumbnail: Option<String>,
    thumbnails: Option<Vec<YtThumbnail>>,
}

#[derive(Debug, Deserialize)]
struct YtThumbnail {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

/// Pick a reasonably-sized thumbnail: largest with both dimensions ≤ 640px,
/// or the largest available if none fit the cap. Falls back to `thumbnail`.
fn pick_thumbnail(info: &YtInfo) -> Option<String> {
    if let Some(list) = info.thumbnails.as_ref().filter(|l| !l.is_empty()) {
        const CAP: u32 = 640;
        let mut under_cap: Vec<&YtThumbnail> = list
            .iter()
            .filter(|t| matches!((t.width, t.height), (Some(w), Some(h)) if w <= CAP && h <= CAP))
            .collect();
        if !under_cap.is_empty() {
            under_cap.sort_by_key(|t| t.width.unwrap_or(0) * t.height.unwrap_or(0));
            return under_cap.last().map(|t| t.url.clone());
        }
        // No sized entries fit the cap — pick the largest sized one, else first.
        let mut sized: Vec<&YtThumbnail> = list
            .iter()
            .filter(|t| t.width.is_some() && t.height.is_some())
            .collect();
        if !sized.is_empty() {
            sized.sort_by_key(|t| t.width.unwrap_or(0) * t.height.unwrap_or(0));
            return sized.first().map(|t| t.url.clone());
        }
        return list.first().map(|t| t.url.clone());
    }
    info.thumbnail.clone()
}

/// Fetch track metadata for a URL (single video, playlist, or search query).
/// Stores the watch-page URL in `path`; stream URL is resolved at play time.
pub fn fetch_tracks(url: &str) -> Result<Vec<Track>> {
    // Expand bare search prefixes to top 5 results
    let resolved = if url.starts_with("ytsearch:")
        && !url[9..].starts_with(|c: char| c.is_ascii_digit())
    {
        format!("ytsearch5:{}", &url[9..])
    } else if url.starts_with("ytmsearch:") && !url[10..].starts_with(|c: char| c.is_ascii_digit())
    {
        format!("ytmsearch5:{}", &url[10..])
    } else if url.starts_with("scsearch:") && !url[9..].starts_with(|c: char| c.is_ascii_digit()) {
        format!("scsearch5:{}", &url[9..])
    } else {
        url.to_string()
    };

    // #69: retry transient failures (network blip, temporary 5xx) before giving up.
    let json_str = with_retry(|| {
        let output = Command::new(yt_dlp_executable())
            .args(["-J", "--flat-playlist", "--no-warnings", &resolved])
            .output()
            .map_err(spawn_error)?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("yt-dlp: {}", err.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    })?;

    let info: YtInfo =
        serde_json::from_str(&json_str).map_err(|e| anyhow!("yt-dlp JSON parse error: {e}"))?;

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
    // Resolve thumbnail before moving fields out of `info` — `pick_thumbnail`
    // borrows `&info`, so this has to happen before the `.or(info.url)` move.
    // Flat-playlist entries (`--flat-playlist`) usually omit thumbnails; fall
    // back to the canonical i.ytimg.com URL derived from the video id for YouTube.
    let is_yt = info
        .webpage_url
        .as_deref()
        .unwrap_or("")
        .contains("youtube")
        || info.url.as_deref().unwrap_or("").contains("youtube");
    let cover_url = pick_thumbnail(&info).or_else(|| {
        if is_yt {
            info.id
                .as_ref()
                .map(|id| format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
        } else {
            None
        }
    });

    // webpage_url preferred; flat-playlist entries use "url"; fallback: construct from id
    let watch_url = if let Some(wp) = info.webpage_url {
        wp
    } else if let Some(u) = info.url {
        if u.starts_with("http://") || u.starts_with("https://") {
            u
        } else if let Some(id) = info.id.as_ref() {
            format!("https://www.youtube.com/watch?v={id}")
        } else {
            format!("https://www.youtube.com/watch?v={u}")
        }
    } else if let Some(id) = info.id {
        format!("https://www.youtube.com/watch?v={id}")
    } else {
        return None;
    };

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
        cover_url,
        added_at: None,
    })
}
