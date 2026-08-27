use anyhow::Result;
use notify::Watcher as _;
use std::{path::PathBuf, time::Duration};

use crate::audio::Track;

use super::{
    types::{LibraryRow, Pane, PlaylistEntry, RepeatMode, UndoSnapshot, ViewMode, MAX_UNDO_SNAPSHOTS},
    util::{pseudo_random, rect_contains, rg_scale},
    App,
};

impl App {
    pub(crate) fn adjust_viz_sensitivity(&mut self, delta: f32) {
        let new_val = self.tap.adjust_sensitivity(delta);
        self.config.visualizer.sensitivity = new_val;
        self.set_info(format!("Visualizer sensitivity: ×{:.1}", new_val));
        if let Err(e) = self.config.save() {
            self.set_info(format!("sensitivity saved in memory only ({e})"));
        }
    }

    pub const AUDIO_PANEL_ROWS: usize = 8;

    pub(crate) fn audio_panel_adjust(&mut self, dir: i32) {
        match self.audio_panel_row {
            0 => {
                self.player.eq().adjust_low(dir as f32);
                let db = self.player.eq().snapshot().low_db;
                self.set_info(format!("EQ Low: {:+.0} dB", db));
            }
            1 => {
                self.player.eq().adjust_mid(dir as f32);
                let db = self.player.eq().snapshot().mid_db;
                self.set_info(format!("EQ Mid: {:+.0} dB", db));
            }
            2 => {
                self.player.eq().adjust_high(dir as f32);
                let db = self.player.eq().snapshot().high_db;
                self.set_info(format!("EQ High: {:+.0} dB", db));
            }
            3 => {
                let presets = crate::eq::PRESETS;
                self.eq_preset_idx = if dir > 0 {
                    (self.eq_preset_idx + 1) % presets.len()
                } else {
                    self.eq_preset_idx.wrapping_sub(1).min(presets.len() - 1)
                };
                let (name, state) = presets[self.eq_preset_idx];
                self.player.eq().set(state);
                self.config.playback.eq_preset = name.to_string();
                let _ = self.config.save();
                self.set_info(format!("EQ Preset: 🎚️ {name}"));
            }
            4 => {
                let v = (self.player.volume() + dir as f32 * 0.05).clamp(0.0, 1.5);
                self.player.set_volume(v);
                self.config.playback.default_volume = v;
                self.set_info(format!("Volume: {}%", (v * 100.0) as u32));
            }
            5 => {
                let xf = (self.player.crossfade_secs + dir as f32 * 0.5).clamp(0.0, 10.0);
                self.player.crossfade_secs = xf;
                self.config.playback.crossfade_secs = xf;
                let _ = self.config.save();
                self.set_info(format!("Crossfade: {:.1}s", xf));
            }
            6 => {
                self.adjust_viz_sensitivity(dir as f32 * crate::visualizer::SENS_STEP);
            }
            7 => {
                let s = (self.player.speed() + dir as f32 * 0.05).clamp(0.5, 2.5);
                self.player.set_speed(s);
                self.set_info(format!("Playback speed: {:.2}×", s));
            }
            _ => {}
        }
    }

    pub(crate) fn cycle_theme(&mut self) {
        if self.theme_names.is_empty() {
            let dir = match crate::config::themes_dir() {
                Ok(d) => d,
                Err(e) => {
                    self.set_info(format!("themes dir: {e}"));
                    return;
                }
            };
            let mut names: Vec<String> = std::fs::read_dir(&dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("toml") {
                        p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            if names.is_empty() {
                names.push("default".to_string());
            }
            self.theme_idx = names
                .iter()
                .position(|n| n == &self.theme.name)
                .unwrap_or(0);
            self.theme_names = names;
        }
        if self.theme_names.is_empty() {
            return;
        }
        self.theme_idx = (self.theme_idx + 1) % self.theme_names.len();
        let name = self.theme_names[self.theme_idx].clone();
        match crate::theme::Theme::load(&name) {
            Ok(theme) => {
                self.config.theme = name.clone();
                self.theme = theme;
                let _ = self.config.save();
                self.set_info(format!("Theme: {name}"));
                self.rearm_theme_watcher();
            }
            Err(e) => self.set_info(format!("Theme error: {e}")),
        }
    }

    pub(crate) fn rearm_theme_watcher(&mut self) {
        let path = match crate::config::themes_dir().map(|d| d.join(format!("{}.toml", self.theme.name))) {
            Ok(p) => p,
            Err(_) => return,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let mut w = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(_) => return,
        };
        if w.watch(&path, notify::RecursiveMode::NonRecursive).is_ok() {
            self.theme_watcher_rx = Some(rx);
            self._theme_watcher = Some(w);
        }
    }

    pub(crate) fn track_is_stream(t: &Track) -> bool {
        let p = t.path.to_string_lossy();
        p.starts_with("http://") || p.starts_with("https://")
    }

    pub(crate) fn seek_relative_async(&mut self, delta_secs: i64) {
        let Some(track) = self.player.current().cloned() else {
            return;
        };
        if !Self::track_is_stream(&track) {
            if let Err(e) = self.player.seek_relative(delta_secs) {
                self.set_info(format!("Seek error: {e}"));
            }
            return;
        }
        let cur_ms = self.player.elapsed().as_millis() as i64;
        let mut new_ms = cur_ms + delta_secs * 1000;
        if new_ms < 0 {
            new_ms = 0;
        }
        if let Some(total) = track.duration {
            let max_ms = total.as_millis().saturating_sub(500) as i64;
            if new_ms > max_ms {
                new_ms = max_ms;
            }
        }
        self.spawn_seek_load(track, Duration::from_millis(new_ms as u64));
    }

    pub(crate) fn seek_fraction_async(&mut self, frac: f32) -> Result<()> {
        let Some(track) = self.player.current().cloned() else {
            return Ok(());
        };
        if !Self::track_is_stream(&track) {
            return self.player.seek_absolute_fraction(frac);
        }
        let Some(total) = track.duration else {
            return Ok(());
        };
        let target_ms = (total.as_millis() as f32 * frac.clamp(0.0, 1.0)) as u64;
        self.spawn_seek_load(track, Duration::from_millis(target_ms));
        Ok(())
    }

    pub(crate) fn seek_to_async(&mut self, target: Duration) {
        let Some(track) = self.player.current().cloned() else {
            return;
        };
        if !Self::track_is_stream(&track) {
            let _ = self.player.seek_to(target);
            return;
        }
        self.spawn_seek_load(track, target);
    }

    pub(crate) fn spawn_seek_load(&mut self, track: Track, offset: Duration) {
        let stream_err = self.player.stream_err_handle();
        let stream_title = self.player.stream_title_handle();
        let (tx, rx) = std::sync::mpsc::channel();
        let t_clone = track.clone();
        std::thread::spawn(move || {
            let result = crate::audio::build_source(&t_clone, offset, stream_err, stream_title)
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.load_rx = Some(rx);
        self.loading_track = Some(track);
        self.pending_seek_offset = Some(offset);
        self.set_info("Seeking…");
    }

    pub(crate) fn pick_next_index(&self, current: usize) -> Option<usize> {
        if self.queue.is_empty() {
            return None;
        }
        if self.shuffle && self.queue.len() > 1 {
            let mut idx = pseudo_random(self.queue.len());
            if idx == current {
                idx = (idx + 1) % self.queue.len();
            }
            Some(idx)
        } else if current + 1 < self.queue.len() {
            Some(current + 1)
        } else if matches!(self.repeat, RepeatMode::All) {
            Some(0)
        } else {
            None
        }
    }

    pub(crate) fn next(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let i = self.queue_index.unwrap_or(0);
        let new = self.pick_next_index(i).unwrap_or(0);
        self.queue_index = Some(new);
        self.queue_state.select(Some(new));

        if let Some(track) = self.queue.get(new).cloned() {
            if let Some(preloaded) = self.prefetch.next.take() {
                if preloaded.path == track.path {
                    self.play_instant(preloaded.source, track);
                    return;
                }
            }
        }
        self.play_current();
    }

    pub(crate) fn prev(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let i = self.queue_index.unwrap_or(0);
        let new = if i == 0 {
            self.queue.len().saturating_sub(1)
        } else {
            i - 1
        };
        self.queue_index = Some(new);
        self.queue_state.select(Some(new));

        if let Some(track) = self.queue.get(new).cloned() {
            if let Some(preloaded) = self.prefetch.prev.take() {
                if preloaded.path == track.path {
                    self.play_instant(preloaded.source, track);
                    return;
                }
            }
        }
        self.play_current();
    }

    pub(crate) fn advance(&mut self) {
        if matches!(self.repeat, RepeatMode::One) {
            self.play_current();
            return;
        }
        if let Some(i) = self.queue_index {
            if let Some(new) = self.pick_next_index(i) {
                self.queue_index = Some(new);
                self.queue_state.select(Some(new));
                if let Some(track) = self.queue.get(new).cloned() {
                    if let Some(preloaded) = self.prefetch.next.take() {
                        if preloaded.path == track.path {
                            self.play_instant(preloaded.source, track);
                            return;
                        }
                    }
                }
                self.play_current();
            } else {
                self.player.stop();
                self.queue_index = None;
                self.prefetch.invalidate();
            }
        }
    }

    pub(crate) fn save_playlist_named(&mut self, name: String) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.set_info(format!("Playlist dir error: {e}"));
                return;
            }
        };
        if std::fs::create_dir_all(&dir).is_err() {
            self.set_info(format!("Could not create {}", dir.display()));
            return;
        }
        let safe_name = if name.is_empty() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("queue-{stamp}")
        } else {
            name.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        let path = dir.join(format!("{safe_name}.m3u"));
        let mut text = String::from("#EXTM3U\n");
        for t in &self.queue {
            let secs = t.duration.map(|d| d.as_secs() as i64).unwrap_or(-1);
            let display = match &t.artist {
                Some(a) if !a.is_empty() => format!("{a} - {}", t.title),
                _ => t.title.clone(),
            };
            let display = display.replace(['\r', '\n'], " ");
            text.push_str(&format!("#EXTINF:{secs},{display}\n"));
            text.push_str(&t.path.display().to_string());
            text.push('\n');
        }
        match std::fs::write(&path, text) {
            Ok(_) => {
                self.active_playlist_name = Some(safe_name.clone());
                self.set_info(format!("Saved: {safe_name}.m3u"));
            }
            Err(e) => self.set_info(format!("Save error: {e}")),
        }
    }

    pub(crate) fn open_playlist_browser(&mut self) {
        let dir = match crate::config::playlists_dir() {
            Ok(p) => p,
            Err(e) => {
                self.set_info(format!("Playlist dir error: {e}"));
                return;
            }
        };
        let mut entries: Vec<PlaylistEntry> = std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("m3u") || ext.eq_ignore_ascii_case("m3u8"))
                    .unwrap_or(false)
            })
            .filter_map(|e| {
                let path = e.path();
                let name = path.file_stem()?.to_str()?.to_string();
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let count = text
                    .lines()
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .count();
                Some(PlaylistEntry {
                    name,
                    path,
                    track_count: count,
                })
            })
            .collect();
        let history = &self.play_history;
        entries.sort_by(|a, b| {
            let la = history
                .playlist_record(&crate::history::PlaylistRef::Local {
                    path: a.path.clone(),
                })
                .map(|r| r.last_played)
                .unwrap_or(0);
            let lb = history
                .playlist_record(&crate::history::PlaylistRef::Local {
                    path: b.path.clone(),
                })
                .map(|r| r.last_played)
                .unwrap_or(0);
            lb.cmp(&la).then_with(|| a.name.cmp(&b.name))
        });
        if entries.is_empty() {
            self.set_info("No playlists saved yet.");
            return;
        }
        self.playlist_browser_entries = entries;
        self.playlist_browser_row = 0;
        self.playlist_browser_delete_confirm = None;
        self.show_playlist_browser = true;
    }

    pub(crate) fn load_playlist_at_row(&mut self, append: bool) {
        let Some(entry) = self
            .playlist_browser_entries
            .get(self.playlist_browser_row)
            .cloned()
        else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&entry.path) else {
            self.set_info(format!("Could not read {}", entry.path.display()));
            return;
        };
        self.push_undo_snapshot(format!(
            "{} playlist '{}'",
            if append { "appended" } else { "loaded" },
            entry.name
        ));
        if !append {
            self.queue.clear();
            self.queue_state.select(None);
            self.queue_index = None;
        }
        let start = self.queue.len();
        let mut pending_extinf: Option<(Option<std::time::Duration>, Option<String>, String)> =
            None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("#EXTINF:") {
                pending_extinf = Some(parse_extinf(rest));
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            let (dur, artist, title) = match pending_extinf.take() {
                Some((d, a, t)) => (d, a, Some(t)),
                None => (None, None, None),
            };
            if line.starts_with("http://") || line.starts_with("https://") {
                self.queue.push(Track::from_url_with_meta(
                    line.to_string(),
                    title,
                    artist,
                    dur,
                ));
            } else {
                let candidate = std::path::Path::new(line);
                let p = if candidate.is_relative() {
                    entry
                        .path
                        .parent()
                        .map(|dir| dir.join(candidate))
                        .unwrap_or_else(|| std::path::PathBuf::from(line))
                } else {
                    std::path::PathBuf::from(line)
                };
                if p.exists() {
                    self.queue.push(Track::from_path_with_meta(p));
                }
            }
        }
        let loaded = self.queue.len() - start;
        if loaded == 0 {
            self.undo_stack.pop_back();
        }
        if !append {
            if !self.queue.is_empty() {
                self.queue_state.select(Some(0));
            }
            self.active_playlist_name = Some(entry.name.clone());
        }
        if loaded > 0 {
            self.play_history.record_playlist_play(
                crate::history::PlaylistRef::Local {
                    path: entry.path.clone(),
                },
                entry.name.clone(),
                None,
                loaded as u32,
            );
        }
        self.show_playlist_browser = false;
        self.set_info(if append {
            format!("Appended {} tracks from '{}'", loaded, entry.name)
        } else {
            format!("Loaded {} tracks from '{}'", loaded, entry.name)
        });
    }

    pub(crate) fn save_eq_preset(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        let snap = self.player.eq().snapshot();
        let preset = crate::config::EqPreset {
            name: name.clone(),
            low_db: snap.low_db,
            mid_db: snap.mid_db,
            high_db: snap.high_db,
        };
        if let Some(existing) = self.custom_eq_presets.iter_mut().find(|p| p.name == name) {
            *existing = preset;
        } else {
            self.custom_eq_presets.push(preset);
        }
        let store = crate::config::EqPresets {
            presets: self.custom_eq_presets.clone(),
        };
        match store.save() {
            Ok(_) => self.set_info(format!("EQ preset '{name}' saved.")),
            Err(e) => self.set_info(format!("EQ preset save error: {e}")),
        }
    }

    pub(crate) fn save_profile(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        let snap = self.player.eq().snapshot();
        let profile = crate::config::Profile {
            name: name.clone(),
            theme: self.config.theme.clone(),
            volume: self.player.volume(),
            shuffle: self.shuffle,
            repeat: matches!(self.repeat, RepeatMode::All | RepeatMode::One),
            eq_low_db: snap.low_db,
            eq_mid_db: snap.mid_db,
            eq_high_db: snap.high_db,
        };
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.name == name) {
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
        let store = crate::config::Profiles {
            profiles: self.profiles.clone(),
        };
        match store.save() {
            Ok(_) => self.set_info(format!("Profile '{name}' saved.")),
            Err(e) => self.set_info(format!("Profile save error: {e}")),
        }
    }

    pub(crate) fn apply_profile(&mut self, idx: usize) {
        let Some(p) = self.profiles.get(idx).cloned() else {
            return;
        };
        self.player.set_volume(p.volume);
        self.config.playback.default_volume = p.volume;
        self.shuffle = p.shuffle;
        self.repeat = if p.repeat {
            RepeatMode::All
        } else {
            RepeatMode::Off
        };
        self.player.eq().set(crate::eq::EqState {
            low_db: p.eq_low_db,
            mid_db: p.eq_mid_db,
            high_db: p.eq_high_db,
        });
        if self.config.theme != p.theme {
            if let Ok(theme) = crate::theme::Theme::load(&p.theme) {
                self.config.theme = p.theme.clone();
                self.theme = theme;
            }
        }
        let _ = self.config.save();
        self.set_info(format!("Profile '{}' loaded.", p.name));
    }

    pub(crate) fn move_selection(&mut self, delta: i32) {
        match self.focus {
            Pane::Library => {
                let rows = self.library_rows();
                if rows.is_empty() {
                    return;
                }
                let mut cur = self.library_state.selected().unwrap_or(0) as i32;
                let len = rows.len() as i32;
                for _ in 0..len {
                    cur = (cur + delta).rem_euclid(len);
                    match &rows[cur as usize] {
                        LibraryRow::Track(_)
                        | LibraryRow::SmartHeader { .. }
                        | LibraryRow::Dir(_) => {
                            self.library_state.select(Some(cur as usize));
                            return;
                        }
                        LibraryRow::Header(_) => {}
                    }
                }
            }
            Pane::Queue => {
                let len = self.queue.len();
                if len == 0 {
                    return;
                }
                let cur = self.queue_state.selected().unwrap_or(0) as i32;
                let new = (cur + delta).rem_euclid(len as i32) as usize;
                self.queue_state.select(Some(new));
            }
        }
    }

    pub(crate) fn activate_selection(&mut self) {
        match self.focus {
            Pane::Library => {
                if self.view_mode == ViewMode::Browser {
                    self.browser_enter();
                    return;
                }
                let sel = self.library_state.selected();
                let row = sel.and_then(|i| self.library_rows().into_iter().nth(i));
                if let Some(LibraryRow::SmartHeader { .. }) = row {
                    if let Some(i) = sel {
                        self.toggle_smart_category(i);
                    }
                    return;
                }
                if let Some(t) = self.selected_library_track() {
                    self.queue.push(t);
                    let idx = self.queue.len() - 1;
                    self.queue_index = Some(idx);
                    self.queue_state.select(Some(idx));
                    self.play_current();
                }
            }
            Pane::Queue => {
                if let Some(i) = self.queue_state.selected() {
                    self.queue_index = Some(i);
                    self.play_current();
                }
            }
        }
    }

    pub(crate) fn enqueue_selection(&mut self) {
        if self.focus == Pane::Library {
            if let Some(t) = self.selected_library_track() {
                self.set_info(format!("Queued: {}", t.display()));
                self.queue.push(t);
                if self.queue_state.selected().is_none() {
                    self.queue_state.select(Some(0));
                }
            }
        }
    }

    pub(crate) fn remove_from_queue(&mut self) {
        if self.focus == Pane::Queue {
            if let Some(i) = self.queue_state.selected() {
                if i < self.queue.len() {
                    let label = format!("removed '{}'", self.queue[i].display());
                    self.push_undo_snapshot(label);
                    self.queue.remove(i);
                    if self.queue_index == Some(i) {
                        self.queue_index = None;
                    } else if self.queue_index.is_some_and(|idx| idx > i) {
                        self.queue_index = self.queue_index.map(|idx| idx - 1);
                    }
                    if self.queue.is_empty() {
                        self.queue_state.select(None);
                    } else {
                        self.queue_state.select(Some(i.min(self.queue.len() - 1)));
                    }
                    self.update_prefetch_slots();
                }
            }
        }
    }

    pub(crate) fn clear_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        let confirmed = self
            .clear_confirm_until
            .map(|until| now < until)
            .unwrap_or(false);
        if !confirmed {
            self.clear_confirm_until = Some(now + Duration::from_secs(3));
            self.set_info(format!(
                "Press c again within 3s to clear {} tracks",
                self.queue.len()
            ));
            return;
        }
        self.clear_confirm_until = None;
        let n = self.queue.len();
        self.push_undo_snapshot(format!("cleared queue ({n} tracks)"));
        self.queue.clear();
        self.queue_state.select(None);
        self.queue_index = None;
        self.prefetch.invalidate();
        self.player.stop();
        self.album_art = None;
        self.art_generation = self.art_generation.wrapping_add(1);
        self.art_picker.invalidate();
        self.set_info(format!("Queue cleared ({n} tracks). Press u to undo."));
    }

    pub(crate) fn push_undo_snapshot(&mut self, label: String) {
        if self.undo_stack.len() >= MAX_UNDO_SNAPSHOTS {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(UndoSnapshot {
            queue: self.queue.clone(),
            queue_index: self.queue_index,
            label,
        });
    }

    pub(crate) fn undo_queue_action(&mut self) {
        let Some(snapshot) = self.undo_stack.pop_back() else {
            self.set_info("Nothing to undo.");
            return;
        };
        self.queue = snapshot.queue;
        self.queue_index = snapshot.queue_index.filter(|&i| i < self.queue.len());
        self.queue_state.select(if self.queue.is_empty() {
            None
        } else {
            Some(self.queue_index.unwrap_or(0).min(self.queue.len() - 1))
        });
        self.update_prefetch_slots();
        self.set_info(format!("Undo: {}", snapshot.label));
    }

    pub(crate) fn play_instant(&mut self, source: crate::audio::SymphoniaSource, track: Track) {
        self.current_play_recorded = false;
        self.lastfm_scrobbled = false;
        self.lastfm_scrobble_info = None;
        self.undo_stack.clear();
        self.load_rx = None;
        self.loading_track = None;
        self.pending_seek_offset = None;

        self.player.rg_scale = rg_scale(&track, self.replaygain_mode);

        match self.player.play_prepared(source, &track, Duration::ZERO) {
            Ok(_) => {
                self.on_track_started(track);
            }
            Err(e) => {
                self.set_error(format!("Playback: {e}"));
                self.play_current();
            }
        }
    }

    pub(crate) fn play_current(&mut self) {
        self.current_play_recorded = false;
        self.lastfm_scrobbled = false;
        self.lastfm_scrobble_info = None;
        self.undo_stack.clear();
        self.prefetch.invalidate();
        let Some(i) = self.queue_index else { return };
        let Some(t) = self.queue.get(i).cloned() else {
            return;
        };

        let path_str = t.path.to_string_lossy();
        if path_str.starts_with("spotify:track:") {
            let uri = path_str.to_string();
            if let Some(api) = self.spotify.as_mut() {
                match api.play_uri(&uri) {
                    Ok(_) => {
                        self.set_info(format!("Spotify ▶ {}", t.display()));
                        self.push_history(t);
                    }
                    Err(e) => self.set_info(format!("Spotify play error: {e}")),
                }
            } else {
                self.set_error("Spotify: not authorized — press Shift+P to login");
            }
            return;
        }

        self.player.rg_scale = rg_scale(&t, self.replaygain_mode);

        let stream_err = self.player.stream_err_handle();
        let stream_title = self.player.stream_title_handle();
        let (tx, rx) = std::sync::mpsc::channel();
        let t_clone = t.clone();
        std::thread::spawn(move || {
            let result = crate::audio::build_source(
                &t_clone,
                std::time::Duration::ZERO,
                stream_err,
                stream_title,
            )
            .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.load_rx = Some(rx);
        self.loading_track = Some(t.clone());
        self.set_info(format!("Loading: {}…", t.display()));
    }
}

    pub(crate) fn toggle_favorite(&mut self) {
        let path = match self.focus {
            Pane::Library => self.selected_library_track().map(|t| t.path),
            Pane::Queue => self
                .queue_state
                .selected()
                .and_then(|i| self.queue.get(i))
                .map(|t| t.path.clone()),
        };
        if let Some(p) = path {
            let fav = self.ratings.toggle_favorite(&p);
            self.set_info(if fav {
                "Added to favorites ♥"
            } else {
                "Removed from favorites"
            });
        }
    }

    pub(crate) fn set_rating(&mut self, rating: u8) {
        let path = match self.focus {
            Pane::Library => self.selected_library_track().map(|t| t.path),
            Pane::Queue => self
                .queue_state
                .selected()
                .and_then(|i| self.queue.get(i))
                .map(|t| t.path.clone()),
        };
        if let Some(p) = path {
            self.ratings.set(&p, rating);
            if rating == 0 {
                self.set_info("Rating removed");
            } else {
                let stars = "★".repeat(rating as usize);
                self.set_info(format!("Rating: {stars} ({rating}/5)"));
            }
        }
    }

    pub(crate) fn parse_extinf(rest: &str) -> (Option<std::time::Duration>, Option<String>, String) {
    let (secs_str, display) = match rest.split_once(',') {
        Some((s, d)) => (s.trim(), d.trim()),
        None => ("", rest.trim()),
    };
    let duration = secs_str
        .parse::<i64>()
        .ok()
        .filter(|s| *s >= 0)
        .map(|s| std::time::Duration::from_secs(s as u64));
    let (artist, title) = match display.split_once(" - ") {
        Some((a, t)) if !a.is_empty() && !t.is_empty() => (Some(a.to_string()), t.to_string()),
        _ => (None, display.to_string()),
    };
    (duration, artist, title)
}

#[cfg(test)]
mod extinf_tests {
    use super::parse_extinf;
    use std::time::Duration;

    #[test]
    fn parses_artist_title_and_duration() {
        let (d, a, t) = parse_extinf("213,Some Artist - Some Title");
        assert_eq!(d, Some(Duration::from_secs(213)));
        assert_eq!(a.as_deref(), Some("Some Artist"));
        assert_eq!(t, "Some Title");
    }

    #[test]
    fn unknown_duration_returns_none() {
        let (d, _, _) = parse_extinf("-1,X - Y");
        assert!(d.is_none());
    }

    #[test]
    fn missing_artist_falls_back_to_title_only() {
        let (_, a, t) = parse_extinf("0,Just a title");
        assert!(a.is_none());
        assert_eq!(t, "Just a title");
    }

    #[test]
    fn handles_payload_without_comma() {
        let (d, a, t) = parse_extinf("only title");
        assert!(d.is_none());
        assert!(a.is_none());
        assert_eq!(t, "only title");
    }
}
