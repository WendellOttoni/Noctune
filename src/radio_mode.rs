//! Radio Mode (#56) — keeps the queue topped up with YouTube search results so
//! a single seed keeps playing forever. The actual fetch reuses `ytdlp::fetch_tracks`
//! and runs in a worker thread; this module owns the state machine that decides
//! *when* to fetch.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioMode {
    pub active: bool,
    /// Free-text seed (genre / vibe / track title used as semantic anchor).
    pub seed: String,
    /// Use the currently playing track's title as the seed instead of `seed`.
    pub smart_seed: bool,
    /// Trigger a refill when the number of upcoming tracks drops to this many.
    pub fetch_threshold: usize,
    /// How many tracks to ask yt-dlp for each refill.
    pub fetch_batch: usize,
    /// Internal — true while a fetch is in flight, prevents stacking requests.
    #[serde(skip)]
    pub is_fetching: bool,
}

impl Default for RadioMode {
    fn default() -> Self {
        Self {
            active: false,
            seed: String::new(),
            smart_seed: false,
            fetch_threshold: 2,
            fetch_batch: 5,
            is_fetching: false,
        }
    }
}

impl RadioMode {
    pub fn query(&self, current_title: Option<&str>) -> String {
        if self.smart_seed {
            if let Some(t) = current_title {
                return format!("ytsearch{}:{} similar music", self.fetch_batch, t);
            }
        }
        format!("ytsearch{}:{} music", self.fetch_batch, self.seed)
    }

    /// True if the queue has reached the refill threshold and a fetch is not already
    /// running. `upcoming_count` is the number of unplayed tracks after `queue_index`.
    pub fn should_fetch(&self, upcoming_count: usize) -> bool {
        self.active && !self.is_fetching && upcoming_count <= self.fetch_threshold
            && (!self.seed.is_empty() || self.smart_seed)
    }
}
