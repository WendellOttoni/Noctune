//! Shareable playlist schema (#79, part of the #78 share epic).
//!
//! This module defines the wire format for playlists that move between
//! Noctune instances or through a future share backend. The format is
//! deliberately decoupled from local filesystem paths: a track is either a
//! `Local` entry (resolvable by metadata against the receiver's library) or
//! a `Stream` entry (carries its own canonical URL).
//!
//! No network or filesystem I/O lives here — just serialisation, conversion
//! to/from M3U, and a fuzzy `resolve` that maps shared tracks back to local
//! `Track`s. The HTTP/UI layers live in their own modules and depend on this
//! one.

pub mod api;
pub mod auth;

use serde::{Deserialize, Serialize};

use crate::audio::Track;

/// Bump this if the schema gains a breaking change. Always serialise the
/// current version on export; on import, refuse versions we don't understand.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Unlisted,
    Private,
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Unlisted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Author {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SharedTrack {
    /// Track that lives in someone else's library. Receiver resolves it via
    /// metadata (artist/title/duration) against their own scan.
    Local {
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artist: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        album: Option<String>,
        /// Track length in milliseconds. Used for fuzzy resolution (±2s window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        /// Optional content hash (`sha256:HEX`) for exact match across libraries.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_hash: Option<String>,
    },
    /// Streaming track with a canonical URL (YouTube, HTTP audio). Resolves
    /// without library lookup — the receiver fetches the URL directly.
    Stream {
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artist: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        source: StreamSource,
        url: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreamSource {
    Youtube,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedPlaylist {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub author: Author,
    /// RFC3339 timestamps. Kept as strings to avoid pulling chrono.
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    pub tracks: Vec<SharedTrack>,
}

impl SharedPlaylist {
    /// Build a new playlist from a slice of local `Track`s. Streaming tracks
    /// are detected by URL prefix on `track.path` — local files become
    /// `SharedTrack::Local`, URLs become `SharedTrack::Stream`.
    pub fn from_tracks(name: impl Into<String>, tracks: &[Track]) -> Self {
        let shared = tracks.iter().map(track_to_shared).collect();
        Self {
            schema_version: SCHEMA_VERSION,
            id: String::new(),
            name: name.into(),
            description: String::new(),
            visibility: Visibility::default(),
            author: Author::default(),
            created_at: String::new(),
            updated_at: String::new(),
            tracks: shared,
        }
    }

    /// Resolve every track against `library`, returning the resolved tracks in
    /// playlist order. Tracks that fail to resolve are returned as
    /// [`ResolvedItem::Missing`] so the UI can surface them.
    pub fn resolve(&self, library: &[Track]) -> Vec<ResolvedItem> {
        self.tracks
            .iter()
            .map(|st| resolve_one(st, library))
            .collect()
    }

    /// Serialise to canonical JSON. Pretty-printed for readability — these
    /// payloads are small and humans regularly look at them.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON. Refuses unknown future schema versions.
    pub fn from_json(s: &str) -> Result<Self, ShareError> {
        let p: Self = serde_json::from_str(s).map_err(ShareError::Json)?;
        if p.schema_version > SCHEMA_VERSION {
            return Err(ShareError::UnsupportedVersion {
                ver: p.schema_version,
            });
        }
        Ok(p)
    }

    /// Render to extended M3U (`#EXTM3U`/`#EXTINF`). Local tracks lose path
    /// information (we have only metadata); the path slot becomes a
    /// `noctune-local://` placeholder so a downstream tool can still tell the
    /// difference between local and stream entries.
    pub fn to_extended_m3u(&self) -> String {
        let mut out = String::from("#EXTM3U\n");
        for t in &self.tracks {
            match t {
                SharedTrack::Local {
                    title,
                    artist,
                    duration_ms,
                    ..
                } => {
                    let dur = duration_ms.map(|d| (d / 1000) as i64).unwrap_or(-1);
                    out.push_str(&format!(
                        "#EXTINF:{dur},{} - {}\n",
                        artist.as_deref().unwrap_or(""),
                        title
                    ));
                    out.push_str("noctune-local://\n");
                }
                SharedTrack::Stream {
                    title,
                    artist,
                    duration_ms,
                    url,
                    ..
                } => {
                    let dur = duration_ms.map(|d| (d / 1000) as i64).unwrap_or(-1);
                    out.push_str(&format!(
                        "#EXTINF:{dur},{} - {}\n",
                        artist.as_deref().unwrap_or(""),
                        title
                    ));
                    out.push_str(url);
                    out.push('\n');
                }
            }
        }
        out
    }

    /// Parse an extended M3U into a `SharedPlaylist`. Entries with HTTP(S) /
    /// YouTube URLs become `Stream` tracks; everything else is treated as a
    /// `Local` track (metadata only — the path is intentionally dropped).
    pub fn from_extended_m3u(name: impl Into<String>, text: &str) -> Self {
        let mut tracks: Vec<SharedTrack> = Vec::new();
        let mut pending: Option<(String, Option<String>, Option<u64>)> = None; // (title, artist, dur_ms)
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("#EXTINF:") {
                // "#EXTINF:217,Artist - Title"
                let (dur_str, after) = rest.split_once(',').unwrap_or((rest, ""));
                let dur_ms: Option<u64> = dur_str
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .filter(|d| *d > 0)
                    .map(|d| (d as u64) * 1000);
                let (artist, title) = match after.split_once(" - ") {
                    Some((a, t)) => (Some(a.trim().to_string()), t.trim().to_string()),
                    None => (None, after.trim().to_string()),
                };
                pending = Some((title, artist, dur_ms));
            } else if !line.is_empty() && !line.starts_with('#') {
                let (title, artist, dur_ms) = pending
                    .take()
                    .unwrap_or_else(|| (line.to_string(), None, None));
                if is_stream_url(line) {
                    let src = if line.contains("youtube.com") || line.contains("youtu.be") {
                        StreamSource::Youtube
                    } else {
                        StreamSource::Http
                    };
                    tracks.push(SharedTrack::Stream {
                        title,
                        artist,
                        duration_ms: dur_ms,
                        source: src,
                        url: line.to_string(),
                    });
                } else {
                    tracks.push(SharedTrack::Local {
                        title,
                        artist,
                        album: None,
                        duration_ms: dur_ms,
                        content_hash: None,
                    });
                }
            }
        }
        Self {
            schema_version: SCHEMA_VERSION,
            id: String::new(),
            name: name.into(),
            description: String::new(),
            visibility: Visibility::default(),
            author: Author::default(),
            created_at: String::new(),
            updated_at: String::new(),
            tracks,
        }
    }
}

/// Result of `SharedPlaylist::resolve` — either a real local track or a
/// placeholder entry the UI should flag as unresolved.
#[derive(Debug, Clone)]
pub enum ResolvedItem {
    Resolved(Track),
    Missing(SharedTrack),
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("invalid JSON: {0}")]
    Json(serde_json::Error),
    #[error("schema version {ver} is newer than supported ({sup})", sup = SCHEMA_VERSION)]
    UnsupportedVersion { ver: u32 },
}

fn is_stream_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn track_to_shared(t: &Track) -> SharedTrack {
    let p = t.path.to_string_lossy();
    if is_stream_url(&p) {
        let src = if p.contains("youtube.com") || p.contains("youtu.be") {
            StreamSource::Youtube
        } else {
            StreamSource::Http
        };
        return SharedTrack::Stream {
            title: t.title.clone(),
            artist: t.artist.clone(),
            duration_ms: t.duration.map(|d| d.as_millis() as u64),
            source: src,
            url: p.to_string(),
        };
    }
    SharedTrack::Local {
        title: t.title.clone(),
        artist: t.artist.clone(),
        album: t.album.clone(),
        duration_ms: t.duration.map(|d| d.as_millis() as u64),
        content_hash: None,
    }
}

const DURATION_FUZZ_MS: u64 = 2000;

fn resolve_one(st: &SharedTrack, library: &[Track]) -> ResolvedItem {
    match st {
        SharedTrack::Stream {
            url,
            title,
            artist,
            duration_ms,
            ..
        } => {
            // Streams resynthesise a Track on the receiver side — we do not
            // try to dedupe against existing queue/library.
            let mut t = synthetic_stream_track(url, title, artist.as_deref(), *duration_ms);
            // Preserve duration if available
            if let Some(ms) = duration_ms {
                t.duration = Some(std::time::Duration::from_millis(*ms));
            }
            ResolvedItem::Resolved(t)
        }
        SharedTrack::Local {
            title,
            artist,
            duration_ms,
            content_hash,
            ..
        } => {
            // Strategy: try exact match by (artist, title, duration±2s); if no
            // duration, fall back to (artist, title); finally, title-only.
            // content_hash is reserved for a future scan-side index — we accept
            // the field but don't yet have a hash store to consult.
            let _ = content_hash;
            let dur = duration_ms.map(std::time::Duration::from_millis);
            let candidate = library.iter().find(|t| {
                eq_ci(&t.title, title)
                    && opt_eq_ci(t.artist.as_deref(), artist.as_deref())
                    && duration_close(t.duration, dur)
            });
            if let Some(t) = candidate {
                return ResolvedItem::Resolved(t.clone());
            }
            // Drop the duration constraint
            let candidate = library.iter().find(|t| {
                eq_ci(&t.title, title) && opt_eq_ci(t.artist.as_deref(), artist.as_deref())
            });
            if let Some(t) = candidate {
                return ResolvedItem::Resolved(t.clone());
            }
            // Title-only last resort
            if let Some(t) = library.iter().find(|t| eq_ci(&t.title, title)) {
                return ResolvedItem::Resolved(t.clone());
            }
            ResolvedItem::Missing(st.clone())
        }
    }
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn opt_eq_ci(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => eq_ci(x, y),
        (None, None) => true,
        // Mixed: artist supplied on one side only — accept it; the title +
        // duration constraints are doing the heavy lifting.
        _ => true,
    }
}

fn duration_close(a: Option<std::time::Duration>, b: Option<std::time::Duration>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => {
            let xm = x.as_millis() as i128;
            let ym = y.as_millis() as i128;
            (xm - ym).unsigned_abs() as u64 <= DURATION_FUZZ_MS
        }
        // Missing duration on either side — don't reject just for that.
        _ => true,
    }
}

fn synthetic_stream_track(
    url: &str,
    title: &str,
    artist: Option<&str>,
    duration_ms: Option<u64>,
) -> Track {
    use std::path::PathBuf;
    Track {
        path: PathBuf::from(url),
        title: title.to_string(),
        artist: artist.map(|s| s.to_string()),
        album: None,
        genre: None,
        year: None,
        duration: duration_ms.map(std::time::Duration::from_millis),
        replaygain_track_db: None,
        replaygain_album_db: None,
        cover_url: None,
        added_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn local_track(title: &str, artist: Option<&str>, dur_ms: Option<u64>) -> Track {
        Track {
            path: PathBuf::from(format!("/music/{title}.mp3")),
            title: title.to_string(),
            artist: artist.map(String::from),
            album: None,
            genre: None,
            year: None,
            duration: dur_ms.map(Duration::from_millis),
            replaygain_track_db: None,
            replaygain_album_db: None,
            cover_url: None,
            added_at: None,
        }
    }

    fn stream_track(url: &str, title: &str) -> Track {
        Track {
            path: PathBuf::from(url),
            title: title.to_string(),
            artist: None,
            album: None,
            genre: None,
            year: None,
            duration: None,
            replaygain_track_db: None,
            replaygain_album_db: None,
            cover_url: None,
            added_at: None,
        }
    }

    #[test]
    fn from_tracks_classifies_local_and_stream() {
        let tracks = vec![
            local_track("Song A", Some("Artist"), Some(200_000)),
            stream_track("https://www.youtube.com/watch?v=abc", "Yt Song"),
            stream_track("https://example.com/stream.mp3", "Http Song"),
        ];
        let p = SharedPlaylist::from_tracks("Mix", &tracks);
        assert_eq!(p.tracks.len(), 3);
        assert!(matches!(p.tracks[0], SharedTrack::Local { .. }));
        assert!(matches!(
            p.tracks[1],
            SharedTrack::Stream {
                source: StreamSource::Youtube,
                ..
            }
        ));
        assert!(matches!(
            p.tracks[2],
            SharedTrack::Stream {
                source: StreamSource::Http,
                ..
            }
        ));
    }

    #[test]
    fn json_round_trip_preserves_order_and_metadata() {
        let tracks = vec![
            local_track("Song A", Some("Artist"), Some(200_000)),
            stream_track("https://www.youtube.com/watch?v=abc", "Yt Song"),
        ];
        let p = SharedPlaylist::from_tracks("Mix", &tracks);
        let json = p.to_json().unwrap();
        let p2 = SharedPlaylist::from_json(&json).unwrap();
        assert_eq!(p2.name, "Mix");
        assert_eq!(p2.tracks.len(), 2);
        assert_eq!(p2.tracks[0], p.tracks[0]);
        assert_eq!(p2.tracks[1], p.tracks[1]);
    }

    #[test]
    fn from_json_rejects_future_versions() {
        let text = r#"{"schema_version":99,"id":"","name":"","tracks":[]}"#;
        assert!(matches!(
            SharedPlaylist::from_json(text),
            Err(ShareError::UnsupportedVersion { ver: 99 })
        ));
    }

    #[test]
    fn m3u_round_trip_preserves_streams() {
        let tracks = vec![
            local_track("Song A", Some("Artist"), Some(200_000)),
            stream_track("https://www.youtube.com/watch?v=abc", "Yt Song"),
        ];
        let p = SharedPlaylist::from_tracks("Mix", &tracks);
        let m3u = p.to_extended_m3u();
        let p2 = SharedPlaylist::from_extended_m3u("Mix", &m3u);
        // Local track survives via metadata
        assert_eq!(p2.tracks.len(), 2);
        match &p2.tracks[0] {
            SharedTrack::Local { title, artist, .. } => {
                assert_eq!(title, "Song A");
                assert_eq!(artist.as_deref(), Some("Artist"));
            }
            _ => panic!("expected Local"),
        }
        // Stream URL survives intact
        match &p2.tracks[1] {
            SharedTrack::Stream { url, source, .. } => {
                assert_eq!(url, "https://www.youtube.com/watch?v=abc");
                assert_eq!(*source, StreamSource::Youtube);
            }
            _ => panic!("expected Stream"),
        }
    }

    #[test]
    fn resolve_exact_metadata_match() {
        let library = vec![local_track("Foo", Some("Bar"), Some(200_000))];
        let p = SharedPlaylist::from_tracks("Mix", &library);
        let resolved = p.resolve(&library);
        assert_eq!(resolved.len(), 1);
        assert!(matches!(resolved[0], ResolvedItem::Resolved(_)));
    }

    #[test]
    fn resolve_within_duration_window() {
        let library = vec![local_track("Foo", Some("Bar"), Some(200_500))];
        let p = SharedPlaylist {
            schema_version: SCHEMA_VERSION,
            id: String::new(),
            name: "Mix".into(),
            description: String::new(),
            visibility: Visibility::default(),
            author: Author::default(),
            created_at: String::new(),
            updated_at: String::new(),
            tracks: vec![SharedTrack::Local {
                title: "Foo".into(),
                artist: Some("Bar".into()),
                album: None,
                duration_ms: Some(199_000), // 1.5s away — within ±2s
                content_hash: None,
            }],
        };
        assert!(matches!(p.resolve(&library)[0], ResolvedItem::Resolved(_)));
    }

    #[test]
    fn resolve_missing_track_is_flagged() {
        let library: Vec<Track> = vec![];
        let p =
            SharedPlaylist::from_tracks("Mix", &[local_track("Nope", Some("Nada"), Some(100_000))]);
        let r = p.resolve(&library);
        assert!(matches!(r[0], ResolvedItem::Missing(_)));
    }

    #[test]
    fn resolve_stream_synthesises_track() {
        let library: Vec<Track> = vec![];
        let p = SharedPlaylist::from_tracks(
            "Mix",
            &[stream_track("https://example.com/song.mp3", "Hosted")],
        );
        match &p.resolve(&library)[0] {
            ResolvedItem::Resolved(t) => assert_eq!(t.title, "Hosted"),
            _ => panic!("stream should resolve without library lookup"),
        }
    }

    #[test]
    fn full_round_trip_local_to_queue() {
        let library = vec![
            local_track("Alpha", Some("X"), Some(180_000)),
            local_track("Beta", Some("Y"), Some(220_000)),
        ];
        let original = SharedPlaylist::from_tracks("Mix", &library);
        let json = original.to_json().unwrap();
        let recovered = SharedPlaylist::from_json(&json).unwrap();
        let queue: Vec<&Track> = recovered
            .resolve(&library)
            .iter()
            .filter_map(|i| match i {
                ResolvedItem::Resolved(t) => Some(t),
                _ => None,
            })
            .map(|t| library.iter().find(|l| l.title == t.title).unwrap())
            .collect();
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].title, "Alpha");
        assert_eq!(queue[1].title, "Beta");
    }
}
