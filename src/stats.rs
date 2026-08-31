use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use crate::{audio::Track, history::PlayHistory};

/// Aggregated playback statistics derived from `PlayHistory` + the in-memory library.
/// Cheap to recompute on demand — no caching needed because PlayHistory is small.
#[derive(Debug, Clone, Default)]
pub struct PlaybackStats {
    pub total_plays: u64,
    pub unique_tracks: usize,
    pub estimated_listen_secs: u64,
    pub top_tracks: Vec<(String, String, u32)>, // (display, path, count)
    pub top_artists: Vec<(String, u32)>,        // (artist, total plays)
    pub recent_tracks: Vec<(String, u64)>,      // (display, last_played)
}

impl PlaybackStats {
    /// `limit` caps the size of each top list.
    pub fn compute(history: &PlayHistory, library: &[Track], limit: usize) -> Self {
        let path_to_track: std::collections::HashMap<String, &Track> = library
            .iter()
            .map(|t| (t.path.display().to_string(), t))
            .collect();

        let top_paths = history.most_played_paths(limit.saturating_mul(2));
        let mut top_tracks: Vec<(String, String, u32)> = top_paths
            .iter()
            .map(|(path, count)| {
                let display = path_to_track
                    .get(path)
                    .map(|t| t.display())
                    .unwrap_or_else(|| path.clone());
                (display, path.clone(), *count)
            })
            .collect();
        top_tracks.truncate(limit);

        let mut artist_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let mut total_plays = 0u64;
        let mut estimated_listen_secs = 0u64;
        let mut unique = std::collections::HashSet::new();
        for (path, count) in history.most_played_paths(usize::MAX) {
            total_plays += count as u64;
            unique.insert(path.clone());
            if let Some(t) = path_to_track.get(&path) {
                if let Some(d) = t.duration {
                    estimated_listen_secs += d.as_secs() * count as u64;
                }
                if let Some(a) = &t.artist {
                    *artist_counts.entry(a.clone()).or_default() += count;
                }
            }
        }
        let mut top_artists: Vec<(String, u32)> = artist_counts.into_iter().collect();
        top_artists.sort_by(|a, b| b.1.cmp(&a.1));
        top_artists.truncate(limit);

        let recent_paths = history.recently_played_paths(limit);
        let recent_tracks: Vec<(String, u64)> = recent_paths
            .into_iter()
            .map(|path| {
                let display = path_to_track
                    .get(&path)
                    .map(|t| t.display())
                    .unwrap_or_else(|| path.clone());
                let last = history.last_played(&std::path::PathBuf::from(&path));
                (display, last)
            })
            .collect();

        Self {
            total_plays,
            unique_tracks: unique.len(),
            estimated_listen_secs,
            top_tracks,
            top_artists,
            recent_tracks,
        }
    }

    pub fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

pub struct SystemStats {
    sys: System,
    networks: Networks,
    pid: Pid,
    pub cpu_pct: f32,
    pub ram_mb: u64,
    pub net_down_kbps: f64,
    last_refresh: Instant,
}

impl SystemStats {
    pub fn new() -> Self {
        let pid = Pid::from_u32(std::process::id());
        let sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new().with_cpu().with_memory()),
        );
        let networks = Networks::new_with_refreshed_list();
        Self {
            sys,
            networks,
            pid,
            cpu_pct: 0.0,
            ram_mb: 0,
            net_down_kbps: 0.0,
            last_refresh: Instant::now() - Duration::from_secs(1),
        }
    }

    pub fn refresh(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refresh);
        if elapsed < Duration::from_secs(1) {
            return;
        }
        self.last_refresh = now;
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );
        if let Some(proc) = self.sys.process(self.pid) {
            self.cpu_pct = proc.cpu_usage();
            self.ram_mb = proc.memory() / 1_048_576;
        }

        self.networks.refresh();
        let bytes: u64 = self.networks.values().map(|n| n.received()).sum();
        self.net_down_kbps = bytes as f64 / 1024.0 / elapsed.as_secs_f64();
    }
}
