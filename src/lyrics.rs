use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub at: Duration,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    pub fn for_track(audio_path: &Path) -> Option<Self> {
        let lrc_path = audio_path.with_extension("lrc");
        let text = fs::read_to_string(&lrc_path).ok()?;
        Self::parse_lrc(&text)
    }

    /// Asynchronous fetch from LRCLIB (#62). Returns None if nothing matches; caller
    /// should treat the result as the final answer (no further retries).
    pub fn fetch_lrclib(
        artist: &str,
        title: &str,
        album: Option<&str>,
        duration: Option<Duration>,
    ) -> Option<Self> {
        if artist.is_empty() || title.is_empty() {
            return None;
        }

        // Cache key: artist+title+(album)+(duration sec). Hits avoid re-querying for
        // tracks we've already looked up.
        let cache_key = lrclib_cache_key(artist, title, album, duration);
        if let Some(path) = lyrics_cache_path(&cache_key) {
            if let Ok(text) = fs::read_to_string(&path) {
                return Self::parse_lrc(&text);
            }
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .ok()?;
        let mut req = client
            .get("https://lrclib.net/api/get")
            .query(&[("artist_name", artist), ("track_name", title)]);
        if let Some(al) = album {
            req = req.query(&[("album_name", al)]);
        }
        if let Some(d) = duration {
            let secs = d.as_secs().to_string();
            req = req.query(&[("duration", secs.as_str())]);
        }
        let resp = req.send().ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: LrclibResponse = resp.json().ok()?;
        let lrc = body.synced_lyrics.or(body.plain_lyrics)?;
        if let Some(path) = lyrics_cache_path(&cache_key) {
            if let Some(parent) = path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    tracing::warn!(target: "lyrics", "failed to create dir {}: {e}", parent.display());
                }
            }
            if let Err(e) = fs::write(&path, &lrc) {
                tracing::warn!(target: "lyrics", "failed to cache {}: {e}", path.display());
            }
        }
        Self::parse_lrc(&lrc)
    }

    fn parse_lrc(text: &str) -> Option<Self> {
        let mut lines: Vec<LyricLine> = Vec::new();

        for raw in text.lines() {
            let line = raw.trim();
            if !line.starts_with('[') {
                continue;
            }

            let mut content = line;
            let mut stamps: Vec<Duration> = Vec::new();
            while let Some(end) = content.find(']') {
                let tag = &content[1..end];
                if let Some(d) = parse_stamp(tag) {
                    stamps.push(d);
                }
                content = &content[end + 1..];
                if !content.starts_with('[') {
                    break;
                }
            }

            let body = content.trim().to_string();
            for d in stamps {
                lines.push(LyricLine {
                    at: d,
                    text: body.clone(),
                });
            }
        }
        lines.sort_by_key(|l| l.at);
        if lines.is_empty() {
            None
        } else {
            Some(Self { lines })
        }
    }

    pub fn current_index(&self, elapsed: Duration) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let mut idx = None;
        for (i, l) in self.lines.iter().enumerate() {
            if l.at <= elapsed {
                idx = Some(i);
            } else {
                break;
            }
        }
        idx
    }
}

#[derive(Debug, Deserialize)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

fn lrclib_cache_key(
    artist: &str,
    title: &str,
    album: Option<&str>,
    duration: Option<Duration>,
) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    artist.to_lowercase().hash(&mut h);
    title.to_lowercase().hash(&mut h);
    album.unwrap_or("").to_lowercase().hash(&mut h);
    duration.map(|d| d.as_secs()).unwrap_or(0).hash(&mut h);
    format!("{:016x}", h.finish())
}

fn lyrics_cache_path(key: &str) -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|p| p.cache_dir().join("lyrics").join(format!("{key}.lrc")))
}

fn parse_stamp(s: &str) -> Option<Duration> {
    let mut parts = s.split(':');
    let mm: u64 = parts.next()?.parse().ok()?;
    let rest = parts.next()?;
    let mut sec_split = rest.split('.');
    let ss: u64 = sec_split.next()?.parse().ok()?;
    let frac_ms = match sec_split.next() {
        Some(f) => {
            let mut n: u64 = f.parse().ok()?;
            match f.len() {
                1 => n *= 100,
                2 => n *= 10,
                _ => {}
            }
            n
        }
        None => 0,
    };
    Some(Duration::from_millis(mm * 60_000 + ss * 1_000 + frac_ms))
}
