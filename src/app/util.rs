//! App-level utility helpers (#94).
//!
//! Free functions extracted from `app.rs` during the modularisation work.
//! No logic change — pure code move.

use crate::audio::Track;

use super::{ReplayGainMode, SortMode};

const ACTIVE_FRAME_MS: u64 = 33;
const IDLE_FRAME_MS: u64 = 100;

pub fn frame_poll_interval(playback_active: bool) -> std::time::Duration {
    std::time::Duration::from_millis(if playback_active {
        ACTIVE_FRAME_MS
    } else {
        IDLE_FRAME_MS
    })
}

pub fn next_queue_index(queue_len: usize, current: usize, repeat_all: bool) -> Option<usize> {
    if queue_len == 0 {
        None
    } else if current + 1 < queue_len {
        Some(current + 1)
    } else if repeat_all {
        Some(0)
    } else {
        None
    }
}

pub fn previous_queue_index(queue_len: usize, current: usize) -> Option<usize> {
    if queue_len == 0 {
        None
    } else if current == 0 {
        Some(queue_len - 1)
    } else {
        Some(current - 1)
    }
}

pub fn rg_scale(track: &Track, mode: ReplayGainMode) -> f32 {
    let db = match mode {
        ReplayGainMode::Off => return 1.0,
        ReplayGainMode::Track => track.replaygain_track_db,
        ReplayGainMode::Album => track.replaygain_album_db.or(track.replaygain_track_db),
    };
    db.map(|db| 10f32.powf(db / 20.0)).unwrap_or(1.0)
}

pub fn parse_spotify_url(url: &str) -> (String, String) {
    if let Some(path) = url.strip_prefix("spotify:") {
        let mut parts = path.splitn(2, ':');
        let k = parts.next().unwrap_or("").to_string();
        let i = parts.next().unwrap_or("").to_string();
        (k, i)
    } else {
        let trimmed = url.split('?').next().unwrap_or(url);
        let segs: Vec<&str> = trimmed.rsplit('/').take(2).collect();
        let i = segs.first().copied().unwrap_or("").to_string();
        let k = segs.get(1).copied().unwrap_or("").to_string();
        (k, i)
    }
}

pub fn rect_contains(r: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    r.width > 0
        && r.height > 0
        && x >= r.x
        && x < r.x.saturating_add(r.width)
        && y >= r.y
        && y < r.y.saturating_add(r.height)
}

/// Convert a terminal column to a fraction of the visible progress bar.
/// `build_progress` reserves one leading and one trailing cell inside the
/// layout rectangle, so mouse hit-testing must use the same inner geometry.
pub fn progress_fraction(r: ratatui::layout::Rect, x: u16) -> f32 {
    let start = r.x.saturating_add(1);
    let visible_width = r.width.saturating_sub(2);
    if visible_width <= 1 {
        return 0.0;
    }
    let position = x.saturating_sub(start).min(visible_width - 1);
    position as f32 / (visible_width - 1) as f32
}

pub fn pseudo_random(modulo: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(1);
    let mut x = nanos
        .wrapping_mul(2862933555777941757)
        .wrapping_add(3037000493);
    x ^= x >> 33;
    (x as usize) % modulo.max(1)
}

pub fn sort_tracks(tracks: &mut [Track], mode: SortMode) {
    sort_tracks_with_ratings(tracks, mode, None);
}

pub fn sort_tracks_with_ratings(
    tracks: &mut [Track],
    mode: SortMode,
    ratings: Option<&crate::ratings::Ratings>,
) {
    let cmp_ci = |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase());

    match mode {
        SortMode::Title => tracks.sort_by(|a, b| cmp_ci(&a.title, &b.title)),
        SortMode::Artist => tracks.sort_by(|a, b| {
            let aa = a.artist.as_deref().unwrap_or("~");
            let bb = b.artist.as_deref().unwrap_or("~");
            cmp_ci(aa, bb).then_with(|| cmp_ci(&a.title, &b.title))
        }),
        SortMode::Album => tracks.sort_by(|a, b| {
            let aa = a.album.as_deref().unwrap_or("~");
            let bb = b.album.as_deref().unwrap_or("~");
            cmp_ci(aa, bb).then_with(|| cmp_ci(&a.title, &b.title))
        }),
        SortMode::Rating => {
            tracks.sort_by(|a, b| {
                let ra = ratings.map(|r| r.get(&a.path)).unwrap_or(0);
                let rb = ratings.map(|r| r.get(&b.path)).unwrap_or(0);
                rb.cmp(&ra).then_with(|| cmp_ci(&a.title, &b.title))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{frame_poll_interval, next_queue_index, previous_queue_index, progress_fraction};
    use ratatui::layout::Rect;
    use std::time::Duration;

    #[test]
    fn uses_fast_frames_during_playback() {
        assert_eq!(frame_poll_interval(true), Duration::from_millis(33));
    }

    #[test]
    fn slows_frames_while_idle() {
        assert_eq!(frame_poll_interval(false), Duration::from_millis(100));
    }

    #[test]
    fn next_queue_index_advances_and_stops_at_end() {
        assert_eq!(next_queue_index(3, 0, false), Some(1));
        assert_eq!(next_queue_index(3, 2, false), None);
    }

    #[test]
    fn next_queue_index_wraps_only_when_repeating_all() {
        assert_eq!(next_queue_index(3, 2, true), Some(0));
        assert_eq!(next_queue_index(0, 0, true), None);
    }

    #[test]
    fn previous_queue_index_wraps_to_last_track() {
        assert_eq!(previous_queue_index(3, 0), Some(2));
        assert_eq!(previous_queue_index(3, 2), Some(1));
        assert_eq!(previous_queue_index(0, 0), None);
    }

    #[test]
    fn progress_fraction_matches_visible_bar_cells() {
        let rect = Rect::new(10, 2, 12, 1);
        assert_eq!(progress_fraction(rect, 11), 0.0);
        assert!((progress_fraction(rect, 15) - 4.0 / 9.0).abs() < f32::EPSILON);
        assert_eq!(progress_fraction(rect, 20), 1.0);
        assert_eq!(progress_fraction(rect, 21), 1.0);
    }
}
