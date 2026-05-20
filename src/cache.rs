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
    /// Unix timestamp of the last time the cache entry was returned to a caller.
    /// Used by `prune` to expire stale entries (#70).
    #[serde(default)]
    pub last_accessed: u64,
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
        let now = now_unix();
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.mtime == mtime && entry.size == size {
                entry.last_accessed = now;
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
            last_accessed: now,
        };
        let track = track_from_cache(path, &entry);
        self.entries.insert(key, entry);
        track
    }

    /// Drop entries that haven't been touched in `expire_days` days (0 disables age
    /// expiration) and trim the cache to fit within `max_size_mb` of serialized JSON
    /// (0 disables size capping). Used by App at startup and save (#70).
    pub fn prune(&mut self, expire_days: u64, max_size_mb: u64) {
        if expire_days > 0 {
            let cutoff = now_unix().saturating_sub(expire_days * 86_400);
            self.entries.retain(|_, e| e.last_accessed == 0 || e.last_accessed >= cutoff);
        }
        if max_size_mb > 0 {
            let budget = max_size_mb.saturating_mul(1024 * 1024) as usize;
            let serialized_len = serde_json::to_string(self).map(|s| s.len()).unwrap_or(0);
            if serialized_len > budget {
                // Drop oldest entries first until we're below the limit. Sorting is O(n log n)
                // but only runs on save and only when actually over-budget.
                let mut by_age: Vec<(String, u64)> = self
                    .entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.last_accessed))
                    .collect();
                by_age.sort_by_key(|(_, ts)| *ts);
                let target = (self.entries.len() as u64)
                    .saturating_mul(budget as u64)
                    .checked_div(serialized_len as u64)
                    .unwrap_or(0) as usize;
                let to_drop = self.entries.len().saturating_sub(target);
                for (k, _) in by_age.into_iter().take(to_drop) {
                    self.entries.remove(&k);
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
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

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
