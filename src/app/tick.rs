use std::{collections::HashSet, path::PathBuf, time::Duration};

use crate::keybinds::Bindings;

use super::{util::sort_tracks, App};

impl App {
    pub(crate) fn poll_service_events(&mut self) {
        let lang = self.config.ui.language;
        while let Ok(event) = self.service_rx.try_recv() {
            match event {
                crate::app::ServiceEvent::SpotifyLogin(result) => match result {
                    Ok(api) => {
                        self.spotify = Some(
                            api.with_legacy_playlists(self.config.spotify.legacy_playlist_api),
                        );
                        self.set_info(
                            lang.text("Spotify login complete.", "Login do Spotify concluído."),
                        );
                    }
                    Err(error) => self.set_error(crate::localized_format!(
                        lang,
                        "Spotify login failed: {error}",
                        "Falha no login do Spotify: {error}"
                    )),
                },
                crate::app::ServiceEvent::SpotifyToggle(result) => match result {
                    Ok((api, message)) => {
                        self.spotify = Some(api);
                        self.set_info(message);
                    }
                    Err(error) => self.set_error(format!("Spotify: {error}")),
                },
                crate::app::ServiceEvent::SpotifyPlaylists(result) => match result {
                    Ok((api, playlists)) => {
                        self.spotify = Some(api);
                        self.spotify_my_playlists = playlists;
                        self.spotify_playlist_row = 0;
                    }
                    Err(error) => self.set_error(format!("Spotify playlists: {error}")),
                },
                crate::app::ServiceEvent::LastfmToken(result) => match result {
                    Ok(token) => {
                        let url = format!(
                            "http://www.last.fm/api/auth/?api_key={}&token={}",
                            self.config.lastfm.api_key, token
                        );
                        let _ = webbrowser::open(&url);
                        self.lastfm_pending_token = Some(token);
                        self.set_info(lang.text(
                            "Last.fm: authorize in browser, then press F again.",
                            "Last.fm: autorize no navegador e pressione F novamente.",
                        ));
                    }
                    Err(error) => self.set_error(crate::localized_format!(
                        lang,
                        "Last.fm token error: {error}",
                        "Erro no token do Last.fm: {error}"
                    )),
                },
                crate::app::ServiceEvent::LastfmSession(result) => match result {
                    Ok((client, username)) => {
                        self.lastfm = Some(client);
                        self.set_info(crate::localized_format!(
                            lang,
                            "Last.fm connected as {username}.",
                            "Last.fm conectado como {username}."
                        ));
                    }
                    Err(error) => self.set_error(crate::localized_format!(
                        lang,
                        "Last.fm auth error: {error}",
                        "Erro na autenticação do Last.fm: {error}"
                    )),
                },
                crate::app::ServiceEvent::BrowseImport(result) => match result {
                    Ok(tracks) => {
                        let count = tracks.len();
                        self.queue.extend(tracks);
                        self.show_browse_modal = false;
                        self.set_info(crate::localized_format!(
                            lang,
                            "Imported {count} tracks into the queue!",
                            "Importadas {count} faixas para a fila!"
                        ));
                    }
                    Err(error) => self.set_error(crate::localized_format!(
                        lang,
                        "Playlist import error: {error}",
                        "Erro ao importar playlist: {error}"
                    )),
                },
                crate::app::ServiceEvent::PlaylistLoaded {
                    result,
                    entry,
                    append,
                } => match result {
                    Ok(tracks) => {
                        let loaded = tracks.len();
                        self.push_undo_snapshot(format!(
                            "{} playlist '{}'",
                            if append {
                                lang.text("appended", "adicionou")
                            } else {
                                lang.text("loaded", "carregou")
                            },
                            entry.name
                        ));
                        if !append {
                            self.queue.clear();
                            self.queue_index = None;
                            self.queue_state.select(None);
                        }
                        self.queue.extend(tracks);
                        if !append && !self.queue.is_empty() {
                            self.queue_state.select(Some(0));
                            self.active_playlist_name = Some(entry.name.clone());
                        }
                        if loaded == 0 {
                            self.undo_stack.pop_back();
                        } else {
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
                            crate::localized_format!(
                                lang,
                                "Appended {loaded} tracks from '{}'",
                                "Adicionadas {loaded} faixas de '{}'",
                                entry.name
                            )
                        } else {
                            crate::localized_format!(
                                lang,
                                "Loaded {loaded} tracks from '{}'",
                                "Carregadas {loaded} faixas de '{}'",
                                entry.name
                            )
                        });
                    }
                    Err(error) => self.set_error(error),
                },
            }
        }
    }

    pub(super) fn poll_plugins(&mut self) {
        let (messages, actions) = if let Some(engine) = &self.plugins {
            engine.set_state(self.player.current(), self.player.volume());
            (engine.drain_messages(), engine.drain_actions())
        } else {
            (Vec::new(), Vec::new())
        };
        for message in messages {
            self.set_info(message);
        }
        for action in actions {
            self.run_action(action);
        }
    }

    pub(super) fn poll_library_scan(&mut self) {
        let lang = self.config.ui.language;
        if let Some(receiver) = &self.scan_progress_rx {
            while let Ok(progress) = receiver.try_recv() {
                self.scan_progress = Some(progress);
            }
        }

        if let Some(receiver) = &self.scan_rx {
            match receiver.try_recv() {
                Ok(scan) => {
                    let mut tracks = scan.tracks;
                    let discovered: HashSet<_> = tracks.iter().map(|t| t.path.clone()).collect();
                    tracks.extend(
                        self.library
                            .iter()
                            .filter(|t| {
                                !discovered.contains(&t.path)
                                    && scan
                                        .unavailable_roots
                                        .iter()
                                        .any(|root| t.path.starts_with(root))
                            })
                            .cloned(),
                    );
                    sort_tracks(&mut tracks, self.sort);
                    let previous_count = self.library.len();
                    let track_count = tracks.len();
                    let live_paths: HashSet<PathBuf> =
                        tracks.iter().map(|track| track.path.clone()).collect();
                    let queue_count = self.queue.len();
                    let keep: Vec<bool> = self
                        .queue
                        .iter()
                        .map(|track| {
                            let path = track.path.to_string_lossy();
                            path.starts_with("http://")
                                || path.starts_with("https://")
                                || path.starts_with("spotify:")
                                || live_paths.contains(&track.path)
                                || scan
                                    .unavailable_roots
                                    .iter()
                                    .any(|root| track.path.starts_with(root))
                        })
                        .collect();
                    self.retain_queue_entries(&keep);
                    let removed_from_queue = queue_count - self.queue.len();
                    self.library = tracks;
                    self.library_revision = self.library_revision.wrapping_add(1);
                    self.library_state.select(if self.library.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                    self.set_info(
                        match (
                            track_count as i64 - previous_count as i64,
                            removed_from_queue,
                        ) {
                            (0, 0) if previous_count > 0 => {
                                crate::localized_format!(
                                    lang,
                                    "Library: {track_count} tracks (unchanged).",
                                    "Biblioteca: {track_count} faixas (sem alterações)."
                                )
                            }
                            (difference, 0) if difference > 0 => {
                                crate::localized_format!(
                                    lang,
                                    "Library: +{difference} → {track_count} tracks.",
                                    "Biblioteca: +{difference} → {track_count} faixas."
                                )
                            }
                            (difference, 0) if difference < 0 => {
                                crate::localized_format!(
                                    lang,
                                    "Library: {difference} → {track_count} tracks.",
                                    "Biblioteca: {difference} → {track_count} faixas."
                                )
                            }
                            (_, removed) if removed > 0 => crate::localized_format!(
                                lang,
                                "Library: {track_count} tracks ({removed} dropped from queue).",
                                "Biblioteca: {track_count} faixas ({removed} removidas da fila)."
                            ),
                            _ => crate::localized_format!(
                                lang,
                                "Library: {track_count} tracks.",
                                "Biblioteca: {track_count} faixas."
                            ),
                        },
                    );
                    self.scan_rx = None;
                    if !scan.unavailable_roots.is_empty() {
                        self.set_error(crate::localized_format!(lang, "Library: {} root(s) unavailable; existing entries preserved. Reconnect and rescan.", "Biblioteca: {} pasta(s) indisponíveis; entradas existentes preservadas. Reconecte e busque novamente.", scan.unavailable_roots.len()));
                    }
                    self.scan_progress_rx = None;
                    self.scan_progress = None;
                    if self.first_run_autoplay {
                        self.first_run_autoplay = false;
                        if let Some(track) = self.library.first().cloned() {
                            self.queue.push(track);
                            self.queue_index = Some(self.queue.len() - 1);
                            self.queue_state.select(self.queue_index);
                            self.play_current();
                        }
                    }
                    if let Some(database) = &self.db {
                        let tracks = self.library.clone();
                        let database = database.clone();
                        std::thread::spawn(move || {
                            let _ = database.sync_tracks(&tracks);
                        });
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.set_error(
                        lang.text("Library: scan failed — check permissions on music_dirs in config.toml", "Biblioteca: falha na busca — confira as permissões de music_dirs no config.toml"),
                    );
                    self.scan_rx = None;
                    self.scan_progress_rx = None;
                    self.scan_progress = None;
                }
            }
        }
    }

    pub(super) fn poll_watchers(&mut self) {
        let lang = self.config.ui.language;
        if let Some(receiver) = &self.fs_event_rx {
            loop {
                match receiver.try_recv() {
                    Ok(Ok(event)) => {
                        let relevant = matches!(
                            event.kind,
                            notify::EventKind::Create(_)
                                | notify::EventKind::Remove(_)
                                | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
                        );
                        if relevant {
                            self.rescan_debounce_until = Some(
                                std::time::Instant::now()
                                    + Duration::from_millis(self.config.library.watch_debounce_ms),
                            );
                        }
                    }
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }

        if let Some(receiver) = &self.config_watcher_rx {
            let mut reload_config = false;
            let mut reload_theme = false;
            let mut reload_presets = false;

            while let Ok(result) = receiver.try_recv() {
                if let Ok(event) = result {
                    for path in event.paths {
                        let file_name = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("");
                        if file_name == "config.toml" {
                            reload_config = true;
                        } else if file_name == "eq_presets.toml" {
                            reload_presets = true;
                        } else if file_name.ends_with(".toml") {
                            reload_theme = true;
                        }
                    }
                }
            }

            if reload_config {
                if let Ok((new_config, warnings)) = crate::config::Config::load_or_default() {
                    let lang = new_config.ui.language;
                    for warning in &warnings {
                        tracing::warn!(target: "config", "hot-reload warning: {warning}");
                    }
                    if new_config.theme != self.theme.name {
                        if let Ok(theme) = crate::theme::Theme::load(&new_config.theme) {
                            self.theme = theme;
                            self.set_info(crate::localized_format!(
                                lang,
                                "Config & Theme: 🎨 {}",
                                "Config & Tema: 🎨 {}",
                                new_config.theme
                            ));
                        }
                    } else {
                        self.set_info(lang.text(
                            "Configuration reloaded (config.toml)",
                            "Configuração recarregada (config.toml)",
                        ));
                    }
                    let (bindings, _) = Bindings::from_config(&new_config.keybinds);
                    self.bindings = bindings;
                    self.player.crossfade_secs = new_config.playback.crossfade_secs;
                    self.config = new_config;
                }
            } else if reload_theme {
                let name = self.theme.name.clone();
                if let Ok(theme) = crate::theme::Theme::load(&name) {
                    self.theme = theme;
                    self.set_info(crate::localized_format!(
                        lang,
                        "Theme reloaded: 🎨 {name}",
                        "Tema recarregado: 🎨 {name}"
                    ));
                }
                self.theme_names = crate::theme::Theme::available_names();
            } else if reload_presets {
                self.custom_eq_presets = crate::config::EqPresets::load().presets;
                self.set_info(lang.text(
                    "Equalizer presets reloaded",
                    "Presets de equalização recarregados",
                ));
            }
        }

        if let Some(deadline) = self.rescan_debounce_until {
            if std::time::Instant::now() >= deadline && self.scan_rx.is_none() {
                self.rescan_debounce_until = None;
                self.start_async_scan();
                self.set_info(lang.text(
                    "Library changed — rescanning…",
                    "Biblioteca alterada — buscando novamente…",
                ));
            }
        }
    }
}
