//! Downloader and offline cache module for streaming tracks (#134).

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::audio::Track;

pub struct DownloadService;

impl DownloadService {
    pub fn start_download(track: Track, dest_dir: PathBuf) -> Receiver<Result<PathBuf, String>> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let res = download_track_worker(&track, &dest_dir);
            let _ = tx.send(res);
        });
        rx
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn download_track_worker(track: &Track, dest_dir: &Path) -> Result<PathBuf, String> {
    if !dest_dir.exists() {
        fs::create_dir_all(dest_dir).map_err(|e| format!("Failed to create dir: {e}"))?;
    }

    let url_str = track.path.to_string_lossy();
    let title = sanitize_filename(&track.title);
    let artist = track
        .artist
        .as_deref()
        .map(sanitize_filename)
        .unwrap_or_default();

    let file_prefix = if !artist.is_empty() {
        format!("{artist} - {title}")
    } else {
        title
    };

    let is_ytdlp_source = url_str.starts_with("https://www.youtube.com")
        || url_str.starts_with("https://youtu.be")
        || url_str.starts_with("https://soundcloud.com")
        || url_str.starts_with("ytsearch:")
        || url_str.starts_with("scsearch:");

    if is_ytdlp_source {
        let output_template = dest_dir.join(format!("{file_prefix}.%(ext)s"));
        let status = Command::new(crate::ytdlp::yt_dlp_executable())
            .args([
                "-x",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "0",
                "--embed-metadata",
                "-o",
                &output_template.to_string_lossy(),
                &url_str,
            ])
            .status()
            .map_err(|e| format!("yt-dlp execution failed: {e}"))?;

        if !status.success() {
            return Err("yt-dlp returned non-zero exit code".to_string());
        }

        let expected_mp3 = dest_dir.join(format!("{file_prefix}.mp3"));
        if expected_mp3.exists() {
            return Ok(expected_mp3);
        }

        if let Ok(entries) = fs::read_dir(dest_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if stem == file_prefix {
                        return Ok(p);
                    }
                }
            }
        }
        return Ok(expected_mp3);
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent("Noctune/1.0 (Audio Player)")
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let mut resp = client
        .get(url_str.as_ref())
        .send()
        .map_err(|e| format!("HTTP request error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP error status: {}", resp.status()));
    }

    let extension = if url_str.contains(".flac") {
        "flac"
    } else if url_str.contains(".ogg") {
        "ogg"
    } else if url_str.contains(".m4a") || url_str.contains(".aac") {
        "m4a"
    } else {
        "mp3"
    };

    let target_path = dest_dir.join(format!("{file_prefix}.{extension}"));
    let mut file = File::create(&target_path)
        .map_err(|e| format!("Failed to create destination file: {e}"))?;

    std::io::copy(&mut resp, &mut file)
        .map_err(|e| format!("Failed to write downloaded stream data: {e}"))?;

    Ok(target_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("Artist / Track : Name? * < > |"),
            "Artist _ Track _ Name_ _ _ _ _"
        );
    }
}
