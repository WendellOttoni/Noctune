use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackRecord {
    pub play_count: u32,
    pub last_played: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PlayHistory {
    entries: HashMap<String, TrackRecord>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl PlayHistory {
    pub fn load() -> Self {
        let path = history_path();
        let mut h: PlayHistory = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        h.path = path;
        h
    }

    pub fn save(&self) {
        let Some(p) = &self.path else { return };
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string(self) {
            let _ = fs::write(p, s);
        }
    }

    pub fn record_play(&mut self, path: &Path) {
        let key = path.display().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let e = self.entries.entry(key).or_default();
        e.play_count += 1;
        e.last_played = now;
        self.save();
    }

    pub fn get(&self, path: &Path) -> TrackRecord {
        self.entries
            .get(&path.display().to_string())
            .cloned()
            .unwrap_or_default()
    }

    pub fn play_count(&self, path: &Path) -> u32 {
        self.get(path).play_count
    }

    pub fn last_played(&self, path: &Path) -> u64 {
        self.get(path).last_played
    }

    /// Paths with non-zero play counts, sorted by count descending.
    pub fn most_played_paths(&self, limit: usize) -> Vec<(String, u32)> {
        let mut v: Vec<(String, u32)> = self
            .entries
            .iter()
            .filter(|(_, r)| r.play_count > 0)
            .map(|(k, r)| (k.clone(), r.play_count))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(limit);
        v
    }

    /// Paths sorted by last_played descending.
    pub fn recently_played_paths(&self, limit: usize) -> Vec<String> {
        let mut v: Vec<(String, u64)> = self
            .entries
            .iter()
            .filter(|(_, r)| r.last_played > 0)
            .map(|(k, r)| (k.clone(), r.last_played))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(limit);
        v.into_iter().map(|(k, _)| k).collect()
    }
}

fn history_path() -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|p| p.config_dir().join("play-history.json"))
}
