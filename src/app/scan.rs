//! Library scan helpers (#94).
//!
//! Extracted from `app.rs` as part of the modularisation work. No logic
//! change — pure code move.

use std::path::PathBuf;
use walkdir::WalkDir;

use crate::{audio::Track, cache::MetadataCache};

use super::AUDIO_EXTS;

#[allow(dead_code)] // kept as the no-progress public entrypoint for #94 follow-ups
pub fn scan_library(dirs: &[PathBuf], cache: &mut MetadataCache) -> Vec<Track> {
    scan_library_with_progress(dirs, cache, None)
}

pub fn scan_library_with_progress(
    dirs: &[PathBuf],
    cache: &mut MetadataCache,
    progress_tx: Option<std::sync::mpsc::Sender<(usize, usize)>>,
) -> Vec<Track> {
    // #88: split the scan into a cheap serial walk that collects paths and a
    // parallel probe phase. The expensive `metadata::probe` is symphonia
    // I/O — embarrassingly parallel, dominates wall time for large libraries.
    // #104: `progress_tx` receives `(done, total)` ticks so the UI status bar
    // can display "Scanning library… [1234/5000]" without blocking.

    let mut paths: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());
            if let Some(e) = ext {
                if AUDIO_EXTS.contains(&e.as_str()) {
                    paths.push(path.to_path_buf());
                }
            }
        }
    }

    // Hand the cache's HashMap to a Mutex while the parallel phase runs.
    // Workers lock briefly to consult/insert; the heavy `probe()` happens
    // outside the critical section.
    use rayon::prelude::*;
    let entries = std::mem::take(&mut cache.entries);
    let cache_mtx = parking_lot::Mutex::new(entries);
    let total = paths.len();
    let done = std::sync::atomic::AtomicUsize::new(0);

    let mut out: Vec<Track> = paths
        .par_iter()
        .map(|path| {
            let key = path.display().to_string();
            let (mtime, size) = crate::cache::file_stat(path);
            let now = crate::cache::now_unix();
            {
                let mut map = cache_mtx.lock();
                if let Some(entry) = map.get_mut(&key) {
                    if entry.mtime == mtime && entry.size == size {
                        entry.last_accessed = now;
                        return crate::cache::track_from_cache(path, entry);
                    }
                }
            }
            // Cache miss — probe under no lock.
            let meta: crate::metadata::TrackMeta = crate::metadata::probe(path);
            let entry = crate::cache::CacheEntry {
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
                added_at: Some(mtime),
            };
            let track = crate::cache::track_from_cache(path, &entry);
            cache_mtx.lock().insert(key, entry);
            // Emit progress every ~32 tracks to avoid flooding the channel
            // on large libraries.
            let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if let Some(tx) = &progress_tx {
                if d == total || d.is_multiple_of(32) {
                    let _ = tx.send((d, total));
                }
            }
            track
        })
        .collect();

    cache.entries = cache_mtx.into_inner();
    out.sort_by_key(|a| a.title.to_lowercase());
    out
}
