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
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!(target: "history", "failed to create dir {}: {e}", parent.display());
            }
        }
        match serde_json::to_string(self) {
            Ok(s) => {
                if let Err(e) = fs::write(p, s) {
                    tracing::warn!(target: "history", "failed to save {}: {e}", p.display());
                }
            }
            Err(e) => tracing::warn!(target: "history", "failed to serialize: {e}"),
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

    #[allow(dead_code)]
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

    /// Drop entries past `retain_days` and trim down to `max_entries`, keeping the
    /// most recently played (#70). 0 on either field disables that check.
    pub fn prune(&mut self, max_entries: usize, retain_days: u64) {
        if retain_days > 0 {
            let cutoff = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .saturating_sub(retain_days * 86_400);
            self.entries
                .retain(|_, r| r.last_played == 0 || r.last_played >= cutoff);
        }
        if max_entries > 0 && self.entries.len() > max_entries {
            let mut by_recent: Vec<(String, u64)> = self
                .entries
                .iter()
                .map(|(k, r)| (k.clone(), r.last_played))
                .collect();
            by_recent.sort_by(|a, b| b.1.cmp(&a.1));
            let keep: std::collections::HashSet<String> = by_recent
                .into_iter()
                .take(max_entries)
                .map(|(k, _)| k)
                .collect();
            self.entries.retain(|k, _| keep.contains(k));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn record_play_increments_count() {
        let mut h = PlayHistory::default();
        let p = PathBuf::from("/music/track.mp3");
        h.record_play(&p);
        h.record_play(&p);
        assert_eq!(h.play_count(&p), 2);
    }

    #[test]
    fn record_play_sets_last_played() {
        let mut h = PlayHistory::default();
        let p = PathBuf::from("/music/track.mp3");
        h.record_play(&p);
        assert!(h.get(&p).last_played > 0);
    }

    #[test]
    fn get_unknown_returns_default() {
        let h = PlayHistory::default();
        let r = h.get(&PathBuf::from("/nope"));
        assert_eq!(r.play_count, 0);
        assert_eq!(r.last_played, 0);
    }

    #[test]
    fn most_played_paths_sorted_desc() {
        let mut h = PlayHistory::default();
        let a = PathBuf::from("/a");
        let b = PathBuf::from("/b");
        h.record_play(&a);
        h.record_play(&b);
        h.record_play(&b);
        h.record_play(&b);
        let top = h.most_played_paths(10);
        assert_eq!(top[0].0, "/b");
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn prune_trims_to_max_entries() {
        let mut h = PlayHistory::default();
        for i in 0..20 {
            h.record_play(&PathBuf::from(format!("/track{i}")));
        }
        h.prune(5, 0);
        assert_eq!(h.entries.len(), 5);
    }

    #[test]
    fn json_round_trip() {
        let mut h = PlayHistory::default();
        h.record_play(&PathBuf::from("/x"));
        let json = serde_json::to_string(&h).unwrap();
        let loaded: PlayHistory = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.play_count(&PathBuf::from("/x")), 1);
    }
}
