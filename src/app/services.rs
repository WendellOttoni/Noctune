use std::{path::PathBuf, time::Duration};

use crate::{
    app::{
        prefetch::SlotKind, scan::scan_library_with_progress, types::Pane, util::parse_spotify_url,
        App,
    },
    audio::Track,
    cache::{cache_path, MetadataCache},
};

impl App {
    pub(crate) fn start_async_scan(&mut self) {
        if self.scan_rx.is_some() {
            self.set_info("Scan already in progress…");
            return;
        }
        let dirs = self.config.music_dirs.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<Track>>();
        let (ptx, prx) = std::sync::mpsc::channel::<(usize, usize)>();
        self.scan_rx = Some(rx);
        self.scan_progress_rx = Some(prx);
        self.scan_progress = None;
        self.set_info("Scanning library…");
        std::thread::spawn(move || {
            let cache_file = cache_path();
            let mut cache = cache_file
                .as_deref()
                .map(MetadataCache::load)
                .unwrap_or_default();
            let tracks = scan_library_with_progress(&dirs, &mut cache, Some(ptx));
            if let Some(p) = &cache_file {
                cache.save(p);
            }
            let _ = tx.send(tracks);
        });
    }

    pub(crate) fn start_url_load(&mut self, url: String) {
        if self.url_rx.is_some() {
            self.set_info("Already loading, please wait…");
            return;
        }

        let lower = url.to_lowercase();
        let is_playlist_file =
            (lower.ends_with(".m3u") || lower.ends_with(".m3u8") || lower.ends_with(".pls"))
                && !url.contains("spotify.com")
                && !url.starts_with("spotify:")
                && !crate::ytdlp::is_youtube_url(&url);

        if is_playlist_file {
            self.set_info("Loading radio playlist…");
            match crate::radio::fetch_playlist(&url) {
                Ok(tracks) if !tracks.is_empty() => {
                    let n = tracks.len();
                    let was_empty = self.queue.is_empty();
                    self.queue.extend(tracks);
                    if was_empty {
                        self.queue_state.select(Some(0));
                    }
                    self.set_info(format!("Added {n} stream(s) from playlist."));
                }
                Ok(_) => self.set_info("Playlist contained no playable streams."),
                Err(e) => self.set_info(format!("Playlist error: {e}")),
            }
            return;
        }

        if !url.contains("spotify.com")
            && !url.starts_with("spotify:")
            && !crate::ytdlp::is_youtube_url(&url)
            && (url.starts_with("http://") || url.starts_with("https://"))
        {
            self.queue.push(crate::audio::Track::from_url(url));
            if self.queue_state.selected().is_none() {
                self.queue_state
                    .select(Some(self.queue.len().saturating_sub(1)));
            }
            self.set_info("Added stream URL to queue.");
            return;
        }

        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && !url.starts_with("spotify:")
            && !crate::ytdlp::is_youtube_url(&url)
        {
            self.set_info(format!("Unrecognised URL scheme: {url}"));
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);

        if url.contains("spotify.com") || url.starts_with("spotify:") {
            let Some(api) = self.spotify.clone() else {
                self.set_error("Spotify: not authorized — press Shift+P to login");
                self.url_rx = None;
                return;
            };
            let (kind, id) = parse_spotify_url(&url);
            self.set_info(format!("Loading Spotify {kind}…"));
            std::thread::spawn(move || {
                let mut api = api;
                let result = match kind.as_str() {
                    "track" => api.track_by_id(&id).map(|t| vec![t]),
                    "playlist" => api.playlist_tracks(&id),
                    "album" => api.album_tracks(&id),
                    other => Err(anyhow::anyhow!("Unsupported Spotify type: {other}")),
                };
                let _ = tx.send(result.map_err(|e| e.to_string()));
            });
            return;
        }

        self.set_info(format!("Loading {url}…"));
        std::thread::spawn(move || {
            let result = crate::ytdlp::fetch_tracks(&url)
                .map_err(|e| e.to_string())
                .and_then(|tracks| {
                    if tracks.is_empty() {
                        Err("yt-dlp returned no tracks for that URL.".into())
                    } else {
                        Ok(tracks)
                    }
                });
            let _ = tx.send(result);
        });
    }

    pub(crate) fn spotify_login(&mut self) {
        if self.spotify_client_id.is_empty() {
            self.set_info("Set [spotify].client_id in config.toml first.");
            return;
        }
        match crate::spotify::authorize(&self.spotify_client_id, &self.spotify_redirect_uri) {
            Ok((url, session)) => {
                self.set_info("Opening browser for Spotify login...");
                let _ = webbrowser::open(&url);
                let port = self
                    .spotify_redirect_uri
                    .rsplit(':')
                    .next()
                    .and_then(|s| s.split('/').next())
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(8888);
                match crate::spotify::oauth::wait_for_redirect(port, &session.state) {
                    Ok(code) => match crate::spotify::exchange_code(&session, &code) {
                        Ok(tokens) => {
                            let _ = crate::spotify::save_tokens(&tokens);
                            match crate::spotify::SpotifyApi::new(
                                self.spotify_client_id.clone(),
                                tokens,
                            ) {
                                Ok(api) => {
                                    self.spotify = Some(api);
                                    self.set_info("Spotify login complete.");
                                }
                                Err(e) => self.set_info(format!("Spotify init error: {e}")),
                            }
                        }
                        Err(e) => self.set_info(format!("Token exchange failed: {e}")),
                    },
                    Err(e) => self.set_info(format!("Redirect listener error: {e}")),
                }
            }
            Err(e) => self.set_info(format!("Spotify authorize error: {e}")),
        }
    }

    pub(crate) fn spotify_toggle(&mut self) {
        let Some(api) = self.spotify.as_mut() else {
            self.set_error("Spotify: not authorized — press Shift+P to login");
            return;
        };
        match api.currently_playing() {
            Ok(Some(cp)) if cp.is_playing => match api.pause() {
                Ok(_) => self.set_info("Spotify paused."),
                Err(e) => self.set_info(format!("Spotify pause error: {e}")),
            },
            Ok(_) => match api.play() {
                Ok(_) => self.set_info("Spotify resumed."),
                Err(e) => self.set_info(format!("Spotify play error: {e}")),
            },
            Err(e) => self.set_info(format!("Spotify error: {e}")),
        }
    }

    pub(crate) fn spotify_load_my_playlists(&mut self) {
        let Some(mut api) = self.spotify.clone() else {
            return;
        };
        match api.my_playlists() {
            Ok(playlists) => {
                self.spotify_my_playlists = playlists;
                if !self.spotify_my_playlists.is_empty() {
                    self.spotify_playlist_row = 0;
                }
            }
            Err(e) => self.set_info(format!("Spotify playlists error: {e}")),
        }
    }

    pub(crate) fn spotify_load_liked(&mut self) {
        let Some(api) = self.spotify.clone() else {
            return;
        };
        self.set_info("Loading liked songs…");
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);
        std::thread::spawn(move || {
            let mut api = api;
            let r = api.liked_tracks(50).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
    }

    pub(crate) fn spotify_load_playlist(&mut self, id: String, name: String) {
        let Some(api) = self.spotify.clone() else {
            return;
        };
        self.set_info(format!("Loading playlist \"{name}\"…"));
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<Vec<Track>, String>>();
        self.url_rx = Some(rx);
        std::thread::spawn(move || {
            let mut api = api;
            let r = api.playlist_tracks(&id).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
    }

    pub(crate) fn spotify_search(&mut self) {
        let query = self.spotify_browser_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let Some(api) = self.spotify.clone() else {
            return;
        };
        self.set_info(format!("Searching Spotify: \"{query}\"…"));
        self.spotify_browser_results.clear();
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<Vec<Track>, String>>();
        self.spotify_search_rx = Some(rx);
        std::thread::spawn(move || {
            let mut api = api;
            let r = api.search(&query, 30).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
    }

    pub(crate) fn subsonic_search(&mut self) {
        let query = self.subsonic_browser_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let client = match crate::subsonic::SubsonicClient::new(&self.config.subsonic) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Subsonic: {e}"));
                return;
            }
        };
        self.set_info(format!("Buscando no Subsonic: \"{query}\"…"));
        self.subsonic_browser_results.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        self.subsonic_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client
                .search(&query)
                .map(crate::subsonic::SubsonicFetchResult::Songs)
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn subsonic_load_tab(&mut self, tab: crate::app::types::SubsonicTab) {
        self.subsonic_browser_tab = tab;
        self.subsonic_browser_row = 0;
        let client = match crate::subsonic::SubsonicClient::new(&self.config.subsonic) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Subsonic: {e}"));
                return;
            }
        };

        let (tx, rx) = std::sync::mpsc::channel();
        self.subsonic_rx = Some(rx);
        match tab {
            crate::app::types::SubsonicTab::Search => {
                if !self.subsonic_browser_query.is_empty() {
                    self.subsonic_search();
                }
            }
            crate::app::types::SubsonicTab::RecentAlbums => {
                self.set_info("Carregando álbuns recentes do Subsonic/Navidrome…");
                self.subsonic_browser_albums.clear();
                std::thread::spawn(move || {
                    let res = client
                        .get_album_list("recent", 40)
                        .map(crate::subsonic::SubsonicFetchResult::Albums)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
            }
            crate::app::types::SubsonicTab::Playlists => {
                self.set_info("Carregando playlists do Subsonic/Navidrome…");
                self.subsonic_browser_playlists.clear();
                std::thread::spawn(move || {
                    let res = client
                        .get_playlists()
                        .map(crate::subsonic::SubsonicFetchResult::Playlists)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
            }
            crate::app::types::SubsonicTab::Random => {
                self.set_info("Obtendo músicas aleatórias do Subsonic/Navidrome…");
                self.subsonic_browser_results.clear();
                std::thread::spawn(move || {
                    let res = client
                        .get_random_songs(50)
                        .map(crate::subsonic::SubsonicFetchResult::Songs)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
            }
        }
    }

    pub(crate) fn subsonic_load_album_tracks(&mut self, album_id: &str) {
        let client = match crate::subsonic::SubsonicClient::new(&self.config.subsonic) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Subsonic: {e}"));
                return;
            }
        };
        let album_id = album_id.to_string();
        self.set_info("Carregando faixas do álbum…");
        let (tx, rx) = std::sync::mpsc::channel();
        self.subsonic_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client
                .get_album_tracks(&album_id)
                .map(crate::subsonic::SubsonicFetchResult::Songs)
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn subsonic_load_playlist_tracks(&mut self, playlist_id: &str) {
        let client = match crate::subsonic::SubsonicClient::new(&self.config.subsonic) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Subsonic: {e}"));
                return;
            }
        };
        let playlist_id = playlist_id.to_string();
        self.set_info("Carregando faixas da playlist…");
        let (tx, rx) = std::sync::mpsc::channel();
        self.subsonic_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client
                .get_playlist_tracks(&playlist_id)
                .map(crate::subsonic::SubsonicFetchResult::Songs)
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn vault_search(&mut self) {
        let query = self.vault_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        let client = match crate::vault::VaultClient::new(&self.config.vault) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Vault: {e}"));
                return;
            }
        };
        self.set_info(format!("Buscando no Cloud Vault: \"{query}\"…"));
        self.vault_results.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        self.vault_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client.search(&query).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn vault_load_recent(&mut self) {
        let client = match crate::vault::VaultClient::new(&self.config.vault) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Vault: {e}"));
                return;
            }
        };
        self.set_info("Carregando catálogo recente do Cloud Vault…");
        self.vault_results.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        self.vault_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client.list_recent(40).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn share_publish_current(&mut self) {
        let title = if self.share_playlist_title.trim().is_empty() {
            self.active_playlist_name
                .clone()
                .unwrap_or_else(|| "Minha Playlist".into())
        } else {
            self.share_playlist_title.trim().to_string()
        };

        if self.queue.is_empty() {
            self.set_error("A fila está vazia. Adicione músicas antes de compartilhar.");
            return;
        }

        let mut pl = crate::share::SharedPlaylist::from_tracks(title, &self.queue);
        pl.description = self.share_playlist_desc.trim().to_string();
        pl.visibility = self.share_playlist_visibility.clone();
        pl.author = crate::share::Author {
            id: "noctune_user".into(),
            display_name: "Noctune User".into(),
        };
        pl.created_at = crate::lastfm::now_unix().to_string();
        pl.updated_at = crate::lastfm::now_unix().to_string();

        let client = match crate::share::api::ShareClient::new(None) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Share: {e}"));
                return;
            }
        };

        self.set_info("Publicando playlist no servidor de descoberta…");
        let (tx, rx) = std::sync::mpsc::channel();
        self.share_publish_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client.publish(&pl, None).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn browse_search_playlists(&mut self) {
        let query = self.browse_search_query.trim().to_string();
        let client = match crate::share::api::ShareClient::new(None) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Share: {e}"));
                return;
            }
        };

        self.set_info(format!("Buscando playlists públicas: \"{query}\"…"));
        self.browse_results.clear();
        let (tx, rx) = std::sync::mpsc::channel();
        self.browse_rx = Some(rx);
        std::thread::spawn(move || {
            let res = client.search(&query, None, 40).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn browse_import_selected(&mut self) {
        let Some(summary) = self.browse_results.get(self.browse_row).cloned() else {
            return;
        };

        let client = match crate::share::api::ShareClient::new(None) {
            Ok(c) => c,
            Err(e) => {
                self.set_error(format!("Share: {e}"));
                return;
            }
        };

        self.set_info(format!("Importando playlist \"{}\"…", summary.name));
        match client.get(&summary.id) {
            Ok(pl) => {
                let resolved_items = pl.resolve(&self.library);
                let mut tracks = Vec::new();
                for item in resolved_items {
                    match item {
                        crate::share::ResolvedItem::Resolved(t) => tracks.push(t),
                        crate::share::ResolvedItem::Missing(st) => {
                            if let crate::share::SharedTrack::Stream {
                                url,
                                title,
                                artist,
                                duration_ms,
                                ..
                            } = st
                            {
                                tracks.push(crate::audio::Track {
                                    path: std::path::PathBuf::from(url),
                                    title,
                                    artist,
                                    album: None,
                                    genre: None,
                                    year: None,
                                    duration: duration_ms.map(std::time::Duration::from_millis),
                                    replaygain_track_db: None,
                                    replaygain_album_db: None,
                                    cover_url: None,
                                    added_at: None,
                                });
                            }
                        }
                    }
                }
                let count = tracks.len();
                self.queue.extend(tracks);
                self.set_info(format!("Importadas {count} faixas para a fila!"));
                self.show_browse_modal = false;
            }
            Err(e) => {
                self.set_error(format!("Erro ao importar playlist: {e}"));
            }
        }
    }

    pub(crate) fn open_tag_editor(&mut self) {
        let track = match self.focus {
            Pane::Library => self.selected_library_track(),
            Pane::Queue => self
                .queue_state
                .selected()
                .and_then(|i| self.queue.get(i))
                .cloned(),
        };
        let Some(t) = track else {
            self.set_info("No track selected to edit tags.");
            return;
        };
        if Self::track_is_stream(&t) {
            self.set_info("Cannot edit tags on stream URLs.");
            return;
        }

        self.tag_editor_path = Some(t.path.clone());
        self.tag_editor_fields = [
            t.title.clone(),
            t.artist.unwrap_or_default(),
            t.album.unwrap_or_default(),
            t.genre.unwrap_or_default(),
            t.year.unwrap_or_default(),
        ];
        self.tag_editor_row = 0;
        self.show_tag_editor = true;
    }

    pub(crate) fn save_tag_editor(&mut self) {
        let Some(path) = self.tag_editor_path.take() else {
            return;
        };
        let title = self.tag_editor_fields[0].trim().to_string();
        let artist = if self.tag_editor_fields[1].trim().is_empty() {
            None
        } else {
            Some(self.tag_editor_fields[1].trim().to_string())
        };
        let album = if self.tag_editor_fields[2].trim().is_empty() {
            None
        } else {
            Some(self.tag_editor_fields[2].trim().to_string())
        };
        let genre = if self.tag_editor_fields[3].trim().is_empty() {
            None
        } else {
            Some(self.tag_editor_fields[3].trim().to_string())
        };
        let year = if self.tag_editor_fields[4].trim().is_empty() {
            None
        } else {
            Some(self.tag_editor_fields[4].trim().to_string())
        };

        for t in &mut self.library {
            if t.path == path {
                t.title = if title.is_empty() {
                    t.title.clone()
                } else {
                    title.clone()
                };
                t.artist = artist.clone();
                t.album = album.clone();
                t.genre = genre.clone();
                t.year = year.clone();
            }
        }
        for t in &mut self.queue {
            if t.path == path {
                t.title = if title.is_empty() {
                    t.title.clone()
                } else {
                    title.clone()
                };
                t.artist = artist.clone();
                t.album = album.clone();
                t.genre = genre.clone();
                t.year = year.clone();
            }
        }
        self.library_revision = self.library_revision.wrapping_add(1);
        self.set_info(format!("Tags updated: {}", title));
    }

    pub(crate) fn open_radio_browser(&mut self) {
        self.show_radio_browser = true;
        self.radio_row = 0;
        self.radio_search_editing = false;
        if self.radio_search_results.is_empty() && self.radio_search_rx.is_none() {
            let cat = crate::radio_browser::RadioCategory::ALL
                .get(self.radio_category_idx)
                .copied()
                .unwrap_or(crate::radio_browser::RadioCategory::TopVoted);
            self.trigger_radio_category_fetch(cat);
        }
    }

    pub(crate) fn trigger_radio_category_fetch(
        &mut self,
        cat: crate::radio_browser::RadioCategory,
    ) {
        use crate::radio_browser::RadioCategory as C;
        match cat {
            C::Curated | C::Custom | C::Favorites => {
                return;
            }
            _ => {}
        }

        let (tx, rx) = std::sync::mpsc::channel();
        self.radio_search_rx = Some(rx);
        let q = self.radio_search_query.trim().to_string();

        std::thread::spawn(move || {
            let res = match cat {
                C::TopVoted => crate::radio_browser::fetch_top_voted(100),
                C::Lofi => crate::radio_browser::fetch_by_tag("lofi", 100),
                C::Rock => crate::radio_browser::fetch_by_tag("rock", 100),
                C::Jazz => crate::radio_browser::fetch_by_tag("jazz", 100),
                C::Synthwave => crate::radio_browser::fetch_by_tag("synthwave", 100),
                C::Brazil => crate::radio_browser::fetch_by_country("brazil", 100),
                C::Classical => crate::radio_browser::fetch_by_tag("classical", 100),
                C::Search => crate::radio_browser::search_radio_browser(&q, 100),
                _ => Ok(Vec::new()),
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn trigger_radio_search(&mut self) {
        let q = self.radio_search_query.trim().to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        self.radio_search_rx = Some(rx);
        self.set_info(format!("Buscando rádios por '{q}'…"));
        std::thread::spawn(move || {
            let res = crate::radio_browser::search_radio_browser(&q, 100).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }

    pub(crate) fn save_custom_radio_station(&mut self) {
        let name = self.radio_custom_fields[0].trim().to_string();
        let url = self.radio_custom_fields[1].trim().to_string();
        let tags = self.radio_custom_fields[2].trim().to_string();

        if url.is_empty() {
            self.set_error("URL da rádio não pode ficar vazia");
            return;
        }

        let final_name = if name.is_empty() {
            url.split('/').last().unwrap_or("Custom Radio").to_string()
        } else {
            name
        };

        let station = crate::radio_browser::RadioStation {
            name: final_name.clone(),
            url: url.clone(),
            homepage: None,
            tags: if tags.is_empty() {
                "custom".into()
            } else {
                tags
            },
            country: Some("Personalizada".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(1),
            stationuuid: None,
        };

        let _ = crate::radio_browser::add_custom_station(station.clone());
        self.radio_curated_list.retain(|s| s.url != url);
        self.radio_curated_list.insert(0, station);

        self.show_radio_custom_modal = false;
        self.radio_custom_fields = [String::new(), String::new(), String::new()];
        self.radio_custom_field_idx = 0;
        self.set_info(format!("Rádio adicionada: {final_name}"));
    }

    pub(crate) fn play_radio_station(
        &mut self,
        station: &crate::radio_browser::RadioStation,
        enqueue: bool,
    ) {
        let mut track = Track::from_url(station.url.clone());
        track.title = station.name.clone();
        track.artist = Some("Radio Stream".into());
        track.genre = Some(station.tags.clone());

        if enqueue {
            let name = station.name.clone();
            self.queue.push(track);
            self.set_info(format!("Enqueued radio: {name}"));
        } else {
            let name = station.name.clone();
            self.queue.push(track);
            let idx = self.queue.len() - 1;
            self.queue_index = Some(idx);
            self.queue_state.select(Some(idx));
            self.play_current();
            self.set_info(format!("Playing radio: {name}"));
        }
    }

    pub(crate) fn handle_self_update(&mut self) {
        if self.is_updating {
            self.set_info("Update already in progress, please wait…");
            return;
        }

        if let Some(info) = &self.update_info {
            if let Some(url) = info.download_url.clone() {
                self.is_updating = true;
                self.set_info(format!("Downloading Noctune v{}…", info.latest_version));
                let (tx, rx) = std::sync::mpsc::channel();
                self.update_apply_rx = Some(rx);
                std::thread::spawn(move || {
                    let res = crate::updater::apply_update(&url).map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
            } else {
                self.set_info(format!(
                    "No automatic binary available for this platform. Please check GitHub release v{}.",
                    info.latest_version
                ));
            }
        } else {
            self.set_info("Checking for new Noctune updates…");
            let (tx, rx) = std::sync::mpsc::channel();
            self.update_check_rx = Some(rx);
            std::thread::spawn(move || {
                let res = crate::updater::check_for_updates().map_err(|e| e.to_string());
                let _ = tx.send(res);
            });
        }
    }

    pub(crate) fn radio_filtered_stations(&self) -> Vec<&crate::radio_browser::RadioStation> {
        let cat = crate::radio_browser::RadioCategory::ALL
            .get(self.radio_category_idx)
            .copied()
            .unwrap_or(crate::radio_browser::RadioCategory::TopVoted);

        match cat {
            crate::radio_browser::RadioCategory::Curated => {
                self.radio_curated_list.iter().collect()
            }
            crate::radio_browser::RadioCategory::Custom => {
                let custom_urls: std::collections::HashSet<String> =
                    crate::radio_browser::load_custom_stations()
                        .into_iter()
                        .map(|s| s.url)
                        .collect();
                self.radio_curated_list
                    .iter()
                    .filter(|st| {
                        custom_urls.contains(&st.url)
                            || st.country.as_deref() == Some("Personalizada")
                            || st.tags.contains("custom")
                    })
                    .collect()
            }
            crate::radio_browser::RadioCategory::Favorites => self
                .radio_curated_list
                .iter()
                .chain(self.radio_search_results.iter())
                .filter(|st| self.ratings.is_favorite(&PathBuf::from(&st.url)))
                .collect(),
            _ => self.radio_search_results.iter().collect(),
        }
    }

    pub(crate) fn open_device_selector(&mut self) {
        self.device_list = crate::audio::enumerate_output_devices();
        let default = crate::audio::default_device_name().unwrap_or_default();
        self.device_selector_row = self
            .device_list
            .iter()
            .position(|n| *n == default)
            .unwrap_or(0);
        self.show_device_selector = true;
    }

    pub(crate) fn lastfm_login(&mut self) {
        let cfg = &self.config.lastfm;
        if !cfg.is_configured() {
            self.set_info("Set [lastfm] api_key and api_secret in config.toml first.");
            return;
        }

        if let Some(token) = self.lastfm_pending_token.take() {
            let api_key = cfg.api_key.clone();
            let api_secret = cfg.api_secret.clone();
            match crate::lastfm::get_session(&api_key, &api_secret, &token) {
                Ok(session) => {
                    let username = session.username.clone();
                    crate::lastfm::save_session(&session);
                    match crate::lastfm::LastfmClient::new(api_key, api_secret, session) {
                        Ok(client) => {
                            self.lastfm = Some(client);
                            self.set_info(format!("Last.fm connected as {username}."));
                        }
                        Err(e) => self.set_info(format!("Last.fm client error: {e}")),
                    }
                }
                Err(e) => {
                    self.set_info(format!("Last.fm auth error: {e}"));
                }
            }
            return;
        }

        let api_key = cfg.api_key.clone();
        let api_secret = cfg.api_secret.clone();
        match crate::lastfm::get_token(&api_key, &api_secret) {
            Ok(token) => {
                let url = format!(
                    "http://www.last.fm/api/auth/?api_key={}&token={}",
                    api_key, token
                );
                let _ = webbrowser::open(&url);
                self.lastfm_pending_token = Some(token);
                self.set_info("Last.fm: authorize in browser, then press F again.");
            }
            Err(e) => self.set_info(format!("Last.fm token error: {e}")),
        }
    }

    pub(crate) fn spawn_lyrics_fetch(&mut self, t: &Track) {
        let artist = t.artist.clone().unwrap_or_default();
        let title = t.title.clone();
        let album = t.album.clone();
        let duration = t.duration;
        let path = t.path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.lyrics_rx = Some(rx);
        std::thread::spawn(move || {
            let result =
                crate::lyrics::Lyrics::fetch_lrclib(&artist, &title, album.as_deref(), duration);
            let _ = tx.send((path, result));
        });
    }

    pub(crate) fn on_track_started(&mut self, t: Track) {
        self.stream_reconnect_attempts = 0;
        let new_art =
            crate::metadata::probe_picture(&t.path).and_then(|bytes| self.art_picker.load(&bytes));
        self.album_art = new_art;
        self.art_generation = self.art_generation.wrapping_add(1);
        self.art_picker.invalidate();
        if self.album_art.is_none() {
            if let Some(url) = t.cover_url.clone() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.art_rx = Some(rx);
                let path = t.path.clone();
                std::thread::spawn(move || {
                    let bytes = crate::metadata::fetch_remote_picture(&url);
                    let _ = tx.send((path, bytes));
                });
            } else {
                self.art_rx = None;
            }
        } else {
            self.art_rx = None;
        }
        self.set_info(format!("Playing: {}", t.display()));
        if let Some(s) = &mut self.media_session {
            s.update_metadata(
                &t.title,
                t.artist.as_deref().unwrap_or(""),
                t.album.as_deref(),
                t.duration,
            );
            s.update_playback(true, Duration::ZERO);
        }
        self.lyrics = crate::lyrics::Lyrics::for_track(&t.path);
        if self.lyrics.is_none() {
            self.spawn_lyrics_fetch(&t);
        }
        let artist = t.artist.clone().unwrap_or_default();
        let title = t.title.clone();
        let ts = crate::lastfm::now_unix();
        self.lastfm_scrobble_info = Some((artist.clone(), title.clone(), ts));
        if let Some(lfm) = self.lastfm.clone() {
            let a = artist.clone();
            let ti = title.clone();
            std::thread::spawn(move || {
                let _ = lfm.update_now_playing(&a, &ti);
            });
        }
        if let Some(tx) = &self.discord_tx {
            let _ = tx.send(crate::discord::Cmd::Update {
                title: title.clone(),
                artist: artist.clone(),
                start_secs: ts as i64,
            });
        }
        if let Some(engine) = &self.plugins {
            engine.trigger_track_start(&t);
        }
        self.push_history(t);
        self.update_prefetch_slots();
    }

    pub(crate) fn update_prefetch_slots(&mut self) {
        let Some(cur_idx) = self.queue_index else {
            self.prefetch.invalidate();
            return;
        };
        if self.queue.is_empty() {
            self.prefetch.invalidate();
            return;
        }

        let next_idx = self.pick_next_index(cur_idx);
        let prev_idx = if cur_idx == 0 {
            if self.queue.len() > 1 {
                Some(self.queue.len() - 1)
            } else {
                None
            }
        } else {
            Some(cur_idx - 1)
        };

        if let Some(n_idx) = next_idx {
            if let Some(target) = self.queue.get(n_idx).cloned() {
                if !Self::track_is_stream(&target) {
                    let path = target.path.clone();
                    let already_ready = self.prefetch.next.as_ref().map(|p| &p.path) == Some(&path);
                    let already_building = self.prefetch.building_next.as_ref() == Some(&path);
                    if !already_ready && !already_building {
                        self.prefetch.next = None;
                        self.prefetch.building_next = Some(path.clone());
                        if let Some(tx) = &self.prefetch.tx {
                            let tx = tx.clone();
                            let stream_err = self.player.stream_err_handle();
                            let stream_title = self.player.stream_title_handle();
                            std::thread::spawn(move || {
                                let res = crate::audio::build_source(
                                    &target,
                                    Duration::ZERO,
                                    stream_err,
                                    stream_title,
                                )
                                .map_err(|e| e.to_string());
                                let _ = tx.send((SlotKind::Next, path, res));
                            });
                        }
                    }
                } else {
                    self.prefetch.next = None;
                    self.prefetch.building_next = None;
                }
            }
        } else {
            self.prefetch.next = None;
            self.prefetch.building_next = None;
        }

        if let Some(p_idx) = prev_idx {
            if let Some(target) = self.queue.get(p_idx).cloned() {
                if !Self::track_is_stream(&target) {
                    let path = target.path.clone();
                    let already_ready = self.prefetch.prev.as_ref().map(|p| &p.path) == Some(&path);
                    let already_building = self.prefetch.building_prev.as_ref() == Some(&path);
                    if !already_ready && !already_building {
                        self.prefetch.prev = None;
                        self.prefetch.building_prev = Some(path.clone());
                        if let Some(tx) = &self.prefetch.tx {
                            let tx = tx.clone();
                            let stream_err = self.player.stream_err_handle();
                            let stream_title = self.player.stream_title_handle();
                            std::thread::spawn(move || {
                                let res = crate::audio::build_source(
                                    &target,
                                    Duration::ZERO,
                                    stream_err,
                                    stream_title,
                                )
                                .map_err(|e| e.to_string());
                                let _ = tx.send((SlotKind::Prev, path, res));
                            });
                        }
                    }
                } else {
                    self.prefetch.prev = None;
                    self.prefetch.building_prev = None;
                }
            }
        } else {
            self.prefetch.prev = None;
            self.prefetch.building_prev = None;
        }
    }

    pub(crate) fn handle_media_event(&mut self, ev: souvlaki::MediaControlEvent) {
        use souvlaki::MediaControlEvent as E;
        match ev {
            E::Play if self.player.is_paused() => {
                self.player.toggle();
            }
            E::Pause if !self.player.is_paused() => {
                self.player.toggle();
            }
            E::Toggle => self.player.toggle(),
            E::Next => self.next(),
            E::Previous => self.prev(),
            E::Stop => self.player.stop(),
            _ => {}
        }
    }

    pub(crate) fn render_overlay_art(&mut self) {
        let Some(img) = self.album_art.as_ref() else {
            return;
        };
        let area = self.layout.art_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        let protocol = self.art_picker.protocol;
        if matches!(protocol, crate::album_art::Protocol::Blocks) {
            return;
        }
        let key = crate::album_art::ArtCacheKey {
            generation: self.art_generation,
            protocol,
            x: area.x,
            y: area.y,
            w: area.width,
            h: area.height,
        };
        let bytes = self.art_picker.cached_overlay(key, || match protocol {
            crate::album_art::Protocol::Kitty => crate::album_art::build_kitty(img, area, 8, 16),
            crate::album_art::Protocol::Iterm2 => crate::album_art::build_iterm2(img, area),
            crate::album_art::Protocol::Blocks => Vec::new(),
        });
        let mut out = std::io::stdout().lock();
        let _ = std::io::Write::write_all(&mut out, bytes);
        let _ = std::io::Write::flush(&mut out);
    }

    pub(crate) fn download_current(&mut self) {
        let Some(track) = self.player.current().cloned() else {
            self.set_info("Nenhuma faixa em reprodução para download.");
            return;
        };

        let path_str = track.path.to_string_lossy();
        if !path_str.starts_with("http://")
            && !path_str.starts_with("https://")
            && !path_str.starts_with("ytsearch:")
            && !path_str.starts_with("scsearch:")
        {
            self.set_info("Esta faixa já é um arquivo local no disco.");
            return;
        }

        if self.download_rx.is_some() {
            self.set_info("Já existe um download em andamento. Aguarde terminar.");
            return;
        }

        let dest_dir = self
            .config
            .music_dirs
            .first()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("./Music"));

        self.set_info(format!("⏳ Iniciando download de '{}'…", track.title));
        let rx = crate::downloader::DownloadService::start_download(track, dest_dir);
        self.download_rx = Some(rx);
    }
}
