use std::path::PathBuf;

use crate::audio::Track;

use super::{
    types::{
        BrowserRowsCache, LibraryRow, LibraryViewCache, LibraryViewFingerprint, SmartRowsCache,
        SortMode, ViewMode, AUDIO_EXTS,
    },
    App,
};

impl App {
    pub fn is_loading(&self) -> bool {
        self.url_rx.is_some() || self.scan_rx.is_some() || self.spotify_search_rx.is_some()
    }

    pub fn search_active(&self) -> bool {
        self.search_editing
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    pub fn visible_library(&self) -> Vec<&Track> {
        let source: Box<dyn Iterator<Item = &Track>> = match self.view_mode {
            ViewMode::RecentlyPlayed => Box::new(self.history.iter()),
            _ => Box::new(self.library.iter()),
        };

        if self.search.is_empty() {
            source.collect()
        } else {
            let needle = self.search.to_lowercase();
            source
                .filter(|t| {
                    let matches = |s: &str| s.to_lowercase().contains(&needle);
                    matches(&t.title)
                        || t.artist.as_deref().map(matches).unwrap_or(false)
                        || t.album.as_deref().map(matches).unwrap_or(false)
                        || t.genre.as_deref().map(matches).unwrap_or(false)
                        || t.year.as_deref().map(matches).unwrap_or(false)
                })
                .collect()
        }
    }

    pub fn library_rows(&mut self) -> Vec<LibraryRow> {
        if self.view_mode == ViewMode::Smart {
            return self.smart_rows_cached().to_vec();
        }
        if self.view_mode == ViewMode::Browser {
            let path = self.browser_current_path();
            let stale = self
                .browser_rows_cache
                .as_ref()
                .map(|cache| cache.path != path)
                .unwrap_or(true);
            if stale {
                self.browser_rows_cache = Some(BrowserRowsCache {
                    path,
                    rows: self.browser_rows(),
                });
            }
            return self
                .browser_rows_cache
                .as_ref()
                .map(|cache| cache.rows.clone())
                .unwrap_or_default();
        }
        self.library_rows_cached().to_vec()
    }

    /// #86: cached non-Smart library rows. Rebuild only when the fingerprint
    /// changes; the render loop otherwise reuses the existing Vec without
    /// re-running `to_lowercase` across the whole library on every frame.
    pub(crate) fn library_rows_cached(&mut self) -> &[LibraryRow] {
        let fp = LibraryViewFingerprint {
            library_revision: self.library_revision,
            history_revision: self.history_revision,
            view_mode: self.view_mode,
            sort: self.sort,
            search: self.search.clone(),
        };
        let stale = match &self.library_view_cache {
            Some(c) => c.fingerprint != fp,
            None => true,
        };
        if stale {
            let rows = self.build_library_rows();
            self.library_view_cache = Some(LibraryViewCache {
                fingerprint: fp,
                rows,
            });
        }
        self.library_view_cache
            .as_ref()
            .map(|c| c.rows.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn build_library_rows(&self) -> Vec<LibraryRow> {
        let visible = self.visible_library();
        if self.view_mode == ViewMode::Flat
            || self.view_mode == ViewMode::RecentlyPlayed
            || self.sort != SortMode::Album
        {
            return visible
                .into_iter()
                .map(|t| LibraryRow::Track(std::sync::Arc::new(t.clone())))
                .collect();
        }
        let mut out = Vec::with_capacity(visible.len() + 16);
        let mut last_album: Option<String> = None;
        for t in visible {
            let album = t.album.clone().unwrap_or_else(|| "Unknown Album".into());
            if last_album.as_deref() != Some(album.as_str()) {
                out.push(LibraryRow::Header(album.clone()));
                last_album = Some(album);
            }
            out.push(LibraryRow::Track(std::sync::Arc::new(t.clone())));
        }
        out
    }

    /// Returns the cached Smart-view rows, rebuilding them only when the
    /// fingerprint (library / history / play-history revisions, expanded
    /// flags) has changed since the last call (#87). The previous version
    /// rebuilt all four categories — and ran `fs::metadata` per library track
    /// for "Recently Added" — every frame.
    pub(crate) fn smart_rows_cached(&mut self) -> &[LibraryRow] {
        let stale = match &self.smart_cache {
            Some(c) => {
                c.library_revision != self.library_revision
                    || c.history_revision != self.history_revision
                    || c.play_history_revision != self.play_history_revision
                    || c.expanded != self.smart_expanded
            }
            None => true,
        };
        if stale {
            let rows = self.build_smart_rows();
            self.smart_cache = Some(SmartRowsCache {
                library_revision: self.library_revision,
                history_revision: self.history_revision,
                play_history_revision: self.play_history_revision,
                expanded: self.smart_expanded,
                rows,
            });
        }
        // Just refreshed (or already fresh) above.
        self.smart_cache
            .as_ref()
            .map(|c| c.rows.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn build_smart_rows(&self) -> Vec<LibraryRow> {
        const LIMIT: usize = 50;
        let track_map: std::collections::HashMap<String, &Track> = self
            .library
            .iter()
            .map(|t| (t.path.display().to_string(), t))
            .collect();

        let most_played: Vec<Track> = {
            let paths = self.play_history.most_played_paths(LIMIT);
            paths
                .iter()
                .filter_map(|(k, _)| track_map.get(k).copied().cloned())
                .collect()
        };

        let recently_played: Vec<Track> = {
            let paths = self.play_history.recently_played_paths(LIMIT);
            paths
                .iter()
                .filter_map(|k| track_map.get(k).copied().cloned())
                .collect()
        };

        let never_played: Vec<Track> = {
            let mut v: Vec<Track> = self
                .library
                .iter()
                .filter(|t| self.play_history.play_count(&t.path) == 0)
                .cloned()
                .collect();
            v.truncate(LIMIT);
            v
        };

        // #87: use the `added_at` field populated at scan time instead of
        // calling `fs::metadata` per track on every frame.
        let recently_added: Vec<Track> = {
            let mut idx: Vec<(usize, u64)> = self
                .library
                .iter()
                .enumerate()
                .map(|(i, t)| (i, t.added_at.unwrap_or(0)))
                .collect();
            idx.sort_by(|a, b| b.1.cmp(&a.1));
            idx.truncate(LIMIT);
            idx.into_iter()
                .map(|(i, _)| self.library[i].clone())
                .collect()
        };

        let categories: [(&str, &Vec<Track>, bool); 4] = [
            ("Most Played", &most_played, self.smart_expanded[0]),
            ("Recently Played", &recently_played, self.smart_expanded[1]),
            ("Recently Added", &recently_added, self.smart_expanded[2]),
            ("Never Played", &never_played, self.smart_expanded[3]),
        ];

        let mut out = Vec::new();
        for (label, tracks, expanded) in categories {
            out.push(LibraryRow::SmartHeader {
                label: label.to_string(),
                count: tracks.len(),
                expanded,
            });
            if expanded {
                for t in tracks.iter() {
                    out.push(LibraryRow::Track(std::sync::Arc::new(t.clone())));
                }
            }
        }
        out
    }

    pub(crate) fn browser_rows(&self) -> Vec<LibraryRow> {
        let dir = if let Some(p) = &self.browser_path {
            p.clone()
        } else if let Some(root) = self.config.music_dirs.get(self.browser_music_root_idx) {
            root.clone()
        } else {
            return Vec::new();
        };

        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut files: Vec<Track> = Vec::new();

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if AUDIO_EXTS.contains(&ext.to_lowercase().as_str()) {
                    files.push(Track::from_path(path));
                }
            }
        }
        dirs.sort();
        files.sort_by_key(|a| a.title.to_lowercase());

        let mut out = Vec::new();
        for d in dirs {
            out.push(LibraryRow::Dir(d));
        }
        for f in files {
            out.push(LibraryRow::Track(std::sync::Arc::new(f)));
        }
        out
    }

    pub fn browser_current_path(&self) -> PathBuf {
        if let Some(p) = &self.browser_path {
            p.clone()
        } else {
            self.config
                .music_dirs
                .get(self.browser_music_root_idx)
                .cloned()
                .unwrap_or_default()
        }
    }

    pub(crate) fn browser_enter(&mut self) {
        let sel = self.library_state.selected().unwrap_or(0);
        let rows = self.library_rows();
        match rows.get(sel) {
            Some(LibraryRow::Dir(p)) => {
                self.browser_path = Some(p.clone());
                self.browser_rows_cache = None;
                self.library_state.select(Some(0));
            }
            Some(LibraryRow::Track(arc)) => {
                let t = (**arc).clone();
                self.queue.push(t);
                let idx = self.queue.len() - 1;
                self.queue_index = Some(idx);
                self.queue_state.select(Some(idx));
                self.play_current();
            }
            _ => {}
        }
    }

    pub(crate) fn browser_up(&mut self) {
        if let Some(current) = &self.browser_path {
            let parent = current.parent().map(|p| p.to_path_buf());
            let is_root = self
                .config
                .music_dirs
                .get(self.browser_music_root_idx)
                .map(|r| current == r)
                .unwrap_or(false);
            if is_root {
                self.browser_path = None;
            } else {
                self.browser_path = parent;
            }
            self.browser_rows_cache = None;
            self.library_state.select(Some(0));
        } else if self.config.music_dirs.len() > 1 {
            self.browser_music_root_idx =
                (self.browser_music_root_idx + 1) % self.config.music_dirs.len();
            self.browser_rows_cache = None;
            self.library_state.select(Some(0));
        }
    }

    pub(crate) fn selected_library_track(&mut self) -> Option<Track> {
        let rows = self.library_rows();
        let idx = self.library_state.selected()?;
        match rows.get(idx)? {
            LibraryRow::Track(arc) => Some((**arc).clone()),
            LibraryRow::Header(_) | LibraryRow::SmartHeader { .. } | LibraryRow::Dir(_) => None,
        }
    }

    pub(crate) fn toggle_smart_category(&mut self, row_idx: usize) {
        let rows = self.library_rows();
        let mut cat_idx = 0usize;
        for (i, row) in rows.iter().enumerate() {
            if let LibraryRow::SmartHeader { .. } = row {
                if i == row_idx {
                    self.smart_expanded[cat_idx] = !self.smart_expanded[cat_idx];
                    return;
                }
                cat_idx += 1;
            }
        }
    }
}
