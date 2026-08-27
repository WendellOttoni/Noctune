//! App-level utility helpers (#94).
//!
//! Free functions extracted from `app.rs` during the modularisation work.
//! No logic change — pure code move.

use crate::audio::Track;

use super::{ReplayGainMode, SortMode};

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
    r.width > 0 && r.height > 0 && x >= r.x && x < r.x.saturating_add(r.width) && y >= r.y && y < r.y.saturating_add(r.height)
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
    let cmp_ci = |a: &str, b: &str| {
        a.to_lowercase().cmp(&b.to_lowercase())
    };

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
