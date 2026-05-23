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

/// Identity of a played playlist (#84). `Local` references a `.m3u` on disk
/// by absolute path; `Shared` is reserved for imported playlists from the
/// future share backend (#80) and carries the originator's id + URL so the
/// entry remains meaningful even after the original local file is gone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PlaylistRef {
    Local {
        path: PathBuf,
    },
    #[allow(dead_code)] // wired up by the share UI (#82/#83)
    Shared {
        id: String,
        server_url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistPlayRecord {
    pub id: PlaylistRef,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub track_count: u32,
    pub last_played: u64,
    pub play_count: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PlayHistory {
    entries: HashMap<String, TrackRecord>,
    /// Per-playlist history (#84). `#[serde(default)]` so existing
    /// `play-history.json` files without this field load cleanly.
    #[serde(default)]
    playlists: Vec<PlaylistPlayRecord>,
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

    // ── Playlist history (#84) ────────────────────────────────────────────

    /// Cap on stored playlist history entries. Oldest get dropped beyond this.
    const MAX_PLAYLIST_ENTRIES: usize = 200;

    /// Record that the user loaded `id` for playback. Increments `play_count`
    /// and bumps `last_played` if the entry already exists; otherwise inserts.
    /// Persists immediately so a crash doesn't lose the last play.
    pub fn record_playlist_play(
        &mut self,
        id: PlaylistRef,
        name: impl Into<String>,
        author: Option<String>,
        track_count: u32,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = name.into();
        if let Some(existing) = self.playlists.iter_mut().find(|p| p.id == id) {
            existing.play_count += 1;
            existing.last_played = now;
            existing.track_count = track_count;
            // Allow renames to stick (user renamed the .m3u file via OS)
            existing.name = name;
            existing.author = author;
        } else {
            self.playlists.push(PlaylistPlayRecord {
                id,
                name,
                author,
                track_count,
                last_played: now,
                play_count: 1,
            });
        }
        // Trim oldest if we crossed the cap. Sort by last_played desc and
        // truncate — cheap on the small N this tops out at.
        if self.playlists.len() > Self::MAX_PLAYLIST_ENTRIES {
            self.playlists
                .sort_by(|a, b| b.last_played.cmp(&a.last_played));
            self.playlists.truncate(Self::MAX_PLAYLIST_ENTRIES);
        }
        self.save();
    }

    /// Most recently played playlists, newest first.
    pub fn recent_playlists(&self, limit: usize) -> Vec<PlaylistPlayRecord> {
        let mut v = self.playlists.clone();
        v.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        v.truncate(limit);
        v
    }

    /// Look up a record by id (used to surface "last played Xd ago" hints
    /// next to playlist browser rows).
    pub fn playlist_record(&self, id: &PlaylistRef) -> Option<&PlaylistPlayRecord> {
        self.playlists.iter().find(|p| &p.id == id)
    }

    /// Remove an entry from the playlist history. Does not touch the
    /// underlying `.m3u` file on disk.
    pub fn forget_playlist(&mut self, id: &PlaylistRef) -> bool {
        let before = self.playlists.len();
        self.playlists.retain(|p| &p.id != id);
        let changed = self.playlists.len() != before;
        if changed {
            self.save();
        }
        changed
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

    // ── Playlist history tests (#84) ──────────────────────────────────────

    fn local(p: &str) -> PlaylistRef {
        PlaylistRef::Local {
            path: PathBuf::from(p),
        }
    }

    #[test]
    fn record_playlist_play_inserts_then_increments() {
        let mut h = PlayHistory::default();
        let id = local("/m/Mix.m3u");
        h.record_playlist_play(id.clone(), "Mix", None, 12);
        h.record_playlist_play(id.clone(), "Mix", None, 12);
        let r = h.playlist_record(&id).unwrap();
        assert_eq!(r.play_count, 2);
        assert!(r.last_played > 0);
    }

    #[test]
    fn recent_playlists_sorted_desc_by_last_played() {
        let mut h = PlayHistory::default();
        h.record_playlist_play(local("/a"), "A", None, 1);
        h.record_playlist_play(local("/b"), "B", None, 1);
        // bump /a so it becomes most recent again
        h.record_playlist_play(local("/a"), "A", None, 1);
        let r = h.recent_playlists(10);
        assert_eq!(r[0].name, "A");
        assert_eq!(r[1].name, "B");
    }

    #[test]
    fn forget_playlist_removes_only_target() {
        let mut h = PlayHistory::default();
        h.record_playlist_play(local("/a"), "A", None, 1);
        h.record_playlist_play(local("/b"), "B", None, 1);
        assert!(h.forget_playlist(&local("/a")));
        assert!(h.playlist_record(&local("/a")).is_none());
        assert!(h.playlist_record(&local("/b")).is_some());
    }

    #[test]
    fn forget_unknown_returns_false() {
        let mut h = PlayHistory::default();
        assert!(!h.forget_playlist(&local("/never-played")));
    }

    #[test]
    fn json_with_unknown_playlists_field_loads_clean() {
        // Old format (no `playlists` field) must still parse — guards against
        // a #[serde(deny_unknown_fields)] regression.
        let old = r#"{"entries":{}}"#;
        let h: PlayHistory = serde_json::from_str(old).unwrap();
        assert!(h.recent_playlists(10).is_empty());
    }

    #[test]
    fn shared_and_local_refs_dont_collide() {
        let mut h = PlayHistory::default();
        let l = PlaylistRef::Local {
            path: PathBuf::from("abc"),
        };
        let s = PlaylistRef::Shared {
            id: "abc".into(),
            server_url: "https://example.com".into(),
        };
        h.record_playlist_play(l.clone(), "L", None, 1);
        h.record_playlist_play(s.clone(), "S", Some("user".into()), 1);
        assert!(h.playlist_record(&l).is_some());
        assert!(h.playlist_record(&s).is_some());
        assert_eq!(h.recent_playlists(10).len(), 2);
    }
}
