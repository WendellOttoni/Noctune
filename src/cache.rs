use crate::audio::Track;
use crate::metadata::{probe, TrackMeta};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheEntry {
    pub mtime: u64,
    pub size: u64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub duration_ms: Option<u64>,
    pub replaygain_track_db: Option<f32>,
    pub replaygain_album_db: Option<f32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MetadataCache {
    pub entries: HashMap<String, CacheEntry>,
}

impl MetadataCache {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string(self) {
            let _ = fs::write(path, s);
        }
    }

    pub fn track_for(&mut self, path: &Path) -> Track {
        let key = path.display().to_string();
        let (mtime, size) = file_stat(path);
        if let Some(entry) = self.entries.get(&key) {
            if entry.mtime == mtime && entry.size == size {
                return track_from_cache(path, entry);
            }
        }

        let meta: TrackMeta = probe(path);
        let entry = CacheEntry {
            mtime,
            size,
            title: meta.title.clone(),
            artist: meta.artist.clone(),
            album: meta.album.clone(),
            genre: meta.genre.clone(),
            year: meta.year.clone(),
            duration_ms: meta.duration.map(|d| d.as_millis() as u64),
            replaygain_track_db: meta.replaygain_track_db,
            replaygain_album_db: meta.replaygain_album_db,
        };
        let track = track_from_cache(path, &entry);
        self.entries.insert(key, entry);
        track
    }
}

fn file_stat(path: &Path) -> (u64, u64) {
    let md = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return (0, 0),
    };
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (mtime, md.len())
}

fn track_from_cache(path: &Path, entry: &CacheEntry) -> Track {
    let fallback = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    Track {
        path: PathBuf::from(path),
        title: entry.title.clone().unwrap_or(fallback),
        artist: entry.artist.clone(),
        album: entry.album.clone(),
        genre: entry.genre.clone(),
        year: entry.year.clone(),
        duration: entry.duration_ms.map(Duration::from_millis),
        replaygain_track_db: entry.replaygain_track_db,
        replaygain_album_db: entry.replaygain_album_db,
    }
}

pub fn cache_path() -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|p| p.cache_dir().join("library.json"))
}

#[allow(dead_code)]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
