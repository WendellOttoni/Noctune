use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::time::Duration;

use crate::{
    app::{
        types::{Pane, SpotifyTab, ViewMode},
        util::{rect_contains, sort_tracks_with_ratings},
        App,
    },
    keybinds::Action,
    ui::util::format_duration,
};

impl App {
    pub(crate) fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return;
        }

        if self.show_help {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                _ => {
                    self.show_help = false;
                    self.help_scroll = 0;
                }
            }
            return;
        }
        if self.show_info {
            self.show_info = false;
            return;
        }

        if self.show_command_palette {
            self.handle_command_palette_key(key);
            return;
        }

        if self.show_audio_panel {
            self.handle_audio_panel_key(key);
            return;
        }

        if self.show_device_selector {
            self.handle_device_selector_key(key);
            return;
        }

        if self.show_eq_tuner {
            self.handle_eq_tuner_key(key);
            return;
        }

        if self.show_playlist_browser {
            self.handle_playlist_browser_key(key);
            return;
        }

        if self.show_profile_browser {
            self.handle_profile_browser_key(key);
            return;
        }

        if self.show_spotify_browser {
            self.handle_spotify_browser_key(key);
            return;
        }

        if self.show_subsonic_browser {
            self.handle_subsonic_browser_key(key);
            return;
        }

        if self.show_tag_editor {
            self.handle_tag_editor_key(key);
            return;
        }

        if self.show_radio_browser {
            self.handle_radio_browser_key(key);
            return;
        }

        if self.show_radio_custom_modal {
            self.handle_radio_custom_modal_key(key);
            return;
        }

        if self.show_lyrics {
            self.handle_lyrics_key(key);
            return;
        }

        if self.eq_preset_name_editing {
            match key.code {
                KeyCode::Esc => {
                    self.eq_preset_name_input.clear();
                    self.eq_preset_name_editing = false;
                }
                KeyCode::Enter => {
                    let name = self.eq_preset_name_input.trim().to_string();
                    self.eq_preset_name_input.clear();
                    self.eq_preset_name_editing = false;
                    self.save_eq_preset(name);
                }
                KeyCode::Backspace => {
                    self.eq_preset_name_input.pop();
                }
                KeyCode::Char(c) => {
                    self.eq_preset_name_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.profile_name_editing {
            match key.code {
                KeyCode::Esc => {
                    self.profile_name_input.clear();
                    self.profile_name_editing = false;
                }
                KeyCode::Enter => {
                    let name = self.profile_name_input.trim().to_string();
                    self.profile_name_input.clear();
                    self.profile_name_editing = false;
                    self.save_profile(name);
                }
                KeyCode::Backspace => {
                    self.profile_name_input.pop();
                }
                KeyCode::Char(c) => {
                    self.profile_name_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.radio_seed_editing {
            match key.code {
                KeyCode::Esc => {
                    self.radio_seed_input.clear();
                    self.radio_seed_editing = false;
                    self.set_info("Radio cancelled.");
                }
                KeyCode::Enter => {
                    let seed = self.radio_seed_input.trim().to_string();
                    self.radio_seed_editing = false;
                    self.radio_seed_input.clear();
                    if seed.is_empty() {
                        self.set_info("Radio: empty seed.");
                    } else {
                        self.radio_mode.seed = seed.clone();
                        self.radio_mode.active = true;
                        self.set_info(format!("Radio: '{seed}' — fetching first batch…"));
                    }
                }
                KeyCode::Backspace => {
                    self.radio_seed_input.pop();
                }
                KeyCode::Char(c) => {
                    self.radio_seed_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.playlist_name_editing {
            match key.code {
                KeyCode::Esc => {
                    self.playlist_name_input.clear();
                    self.playlist_name_editing = false;
                }
                KeyCode::Enter => {
                    let name = self.playlist_name_input.trim().to_string();
                    self.playlist_name_input.clear();
                    self.playlist_name_editing = false;
                    self.save_playlist_named(name);
                }
                KeyCode::Backspace => {
                    self.playlist_name_input.pop();
                }
                KeyCode::Char(c) => {
                    self.playlist_name_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.url_editing {
            match key.code {
                KeyCode::Esc => {
                    self.url_input.clear();
                    self.url_editing = false;
                }
                KeyCode::Enter => {
                    let url = self.url_input.trim().to_string();
                    self.url_input.clear();
                    self.url_editing = false;
                    if !url.is_empty() {
                        self.start_url_load(url);
                    }
                }
                KeyCode::Backspace => {
                    self.url_input.pop();
                }
                KeyCode::Char(c) => {
                    self.url_input.push(c);
                }
                _ => {}
            }
            return;
        }

        if self.search_editing {
            match key.code {
                KeyCode::Esc => {
                    self.search.clear();
                    self.search_editing = false;
                }
                KeyCode::Enter => {
                    self.search_editing = false;
                }
                KeyCode::Backspace => {
                    self.search.pop();
                }
                KeyCode::Char(c) => {
                    self.search.push(c);
                }
                _ => {}
            }
            return;
        }

        // Dedicated Radio View Keys
        if self.view_mode == ViewMode::Radio {
            match key.code {
                KeyCode::Tab => {
                    self.radio_focus_pane = (self.radio_focus_pane + 1) % 2;
                    return;
                }
                KeyCode::BackTab => {
                    self.radio_focus_pane = if self.radio_focus_pane == 0 { 1 } else { 0 };
                    return;
                }
                KeyCode::Char('/') => {
                    self.radio_category_idx = crate::radio_browser::RadioCategory::ALL
                        .iter()
                        .position(|c| *c == crate::radio_browser::RadioCategory::Search)
                        .unwrap_or(0);
                    self.radio_search_editing = true;
                    self.radio_focus_pane = 1;
                    return;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.radio_focus_pane == 0 {
                        self.radio_category_idx = self.radio_category_idx.saturating_sub(1);
                        self.radio_row = 0;
                    } else {
                        self.radio_row = self.radio_row.saturating_sub(1);
                    }
                    return;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.radio_focus_pane == 0 {
                        let max = crate::radio_browser::RadioCategory::ALL.len().saturating_sub(1);
                        if self.radio_category_idx < max {
                            self.radio_category_idx += 1;
                            self.radio_row = 0;
                        }
                    } else {
                        let list = self.radio_filtered_stations();
                        if !list.is_empty() && self.radio_row + 1 < list.len() {
                            self.radio_row += 1;
                        }
                    }
                    return;
                }
                KeyCode::Enter => {
                    let list = self.radio_filtered_stations();
                    if let Some(st) = list.get(self.radio_row).copied() {
                        let st_cloned = st.clone();
                        self.play_radio_station(&st_cloned, false);
                    }
                    return;
                }
                KeyCode::Char('a') => {
                    let list = self.radio_filtered_stations();
                    if let Some(st) = list.get(self.radio_row).copied() {
                        let st_cloned = st.clone();
                        self.play_radio_station(&st_cloned, true);
                    }
                    return;
                }
                KeyCode::Char('+') | KeyCode::Char('N') | KeyCode::Char('n') => {
                    self.show_radio_custom_modal = true;
                    self.radio_custom_fields = [String::new(), String::new(), String::new()];
                    self.radio_custom_field_idx = 0;
                    return;
                }
                KeyCode::Char('f') => {
                    let list = self.radio_filtered_stations();
                    if let Some(st) = list.get(self.radio_row).copied() {
                        let p = std::path::PathBuf::from(&st.url);
                        let is_now_fav = self.ratings.toggle_favorite(&p);
                        let name = st.name.clone();
                        self.set_info(if is_now_fav {
                            format!("Rádio favoritada: {} ♥", name)
                        } else {
                            format!("Rádio removida dos favoritos: {}", name)
                        });
                    }
                    return;
                }
                _ => {}
            }
        }

        if let Some(action) = self.bindings.resolve(&key) {
            self.run_action(action);
        }
    }

    pub(crate) fn run_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Help => self.show_help = true,
            Action::Search => {
                self.search_editing = true;
                self.focus = Pane::Library;
            }
            Action::Tab => {
                self.focus = match self.focus {
                    Pane::Library => Pane::Queue,
                    Pane::Queue => Pane::Library,
                };
            }
            Action::PlayPause => self.player.toggle(),
            Action::Next => self.next(),
            Action::Prev => self.prev(),
            Action::Stop => {
                self.player.stop();
                if let Some(tx) = &self.discord_tx {
                    let _ = tx.send(crate::discord::Cmd::Clear);
                }
            }
            Action::Shuffle => {
                self.shuffle = !self.shuffle;
                self.update_prefetch_slots();
                self.set_info(format!(
                    "Shuffle: {}",
                    if self.shuffle { "on" } else { "off" }
                ));
            }
            Action::Repeat => {
                self.repeat = self.repeat.cycle();
                self.update_prefetch_slots();
                self.set_info(format!("Repeat: {}", self.repeat.label()));
            }
            Action::Sort => {
                self.sort = self.sort.cycle();
                sort_tracks_with_ratings(&mut self.library, self.sort, Some(&self.ratings));
                self.library_state.select(if self.library.is_empty() {
                    None
                } else {
                    Some(0)
                });
                self.set_info(format!("Sort: {}", self.sort.label()));
            }
            Action::SleepTimer => self.toggle_sleep_timer(),
            Action::SavePlaylist => {
                self.playlist_name_editing = true;
                self.playlist_name_input.clear();
                self.set_info("Playlist name (Enter to save, Esc to cancel):");
            }
            Action::LoadPlaylist => {
                self.open_playlist_browser();
            }
            Action::Profiles => {
                self.show_profile_browser = true;
                self.profile_browser_row = 0;
            }
            Action::ShowStats => {
                self.show_stats = !self.show_stats;
            }
            Action::LastfmPanel => {
                if !self.show_lastfm_panel {
                    if let Some(lfm) = self.lastfm.clone() {
                        let (tx, rx) = std::sync::mpsc::channel();
                        self.lastfm_panel_rx = Some(rx);
                        std::thread::spawn(move || {
                            let recent = lfm.recent_tracks(10).unwrap_or_default();
                            let top = lfm.top_artists("1month", 10).unwrap_or_default();
                            let _ = tx.send((recent, top));
                        });
                        self.show_lastfm_panel = true;
                        self.set_info("Last.fm: loading…");
                    } else {
                        self.set_error("Last.fm: not logged in — press Shift+F to authenticate");
                    }
                } else {
                    self.show_lastfm_panel = false;
                }
            }
            Action::RadioMode => {
                if self.radio_mode.active {
                    self.radio_mode.active = false;
                    self.set_info("Radio Mode off.");
                } else {
                    self.radio_seed_editing = true;
                    self.radio_seed_input = self.radio_mode.seed.clone();
                    self.set_info("Radio seed (Enter=confirm, Esc=cancel):");
                }
            }
            Action::SpotifyBrowser => {
                if self.spotify.is_none() {
                    self.set_error("Spotify: not authorized — press Shift+P to login");
                } else {
                    self.show_spotify_browser = true;
                    self.spotify_browser_tab = SpotifyTab::Search;
                    self.spotify_browser_query_editing = true;
                    if self.spotify_my_playlists.is_empty() {
                        self.spotify_load_my_playlists();
                    }
                }
            }
            Action::VolumeUp => {
                let v = (self.player.volume() + 0.05).min(1.5);
                self.player.set_volume(v);
                self.config.playback.default_volume = v;
            }
            Action::VolumeDown => {
                let v = (self.player.volume() - 0.05).max(0.0);
                self.player.set_volume(v);
                self.config.playback.default_volume = v;
            }
            Action::SeekBack => {
                self.seek_relative_async(-5);
            }
            Action::SeekForward => {
                self.seek_relative_async(5);
            }
            Action::SelectionUp => self.move_selection(-1),
            Action::SelectionDown => self.move_selection(1),
            Action::ActivateSelection => self.activate_selection(),
            Action::Enqueue => self.enqueue_selection(),
            Action::RemoveQueueItem => self.remove_from_queue(),
            Action::ClearQueue => self.clear_queue(),
            Action::SpotifyLogin => self.spotify_login(),
            Action::SpotifyToggle => self.spotify_toggle(),
            Action::ToggleView => {
                self.view_mode = self.view_mode.toggle();
                if self.view_mode == ViewMode::Browser {
                    self.browser_path = None;
                    self.browser_music_root_idx = 0;
                }
                self.library_state.select(Some(0));
                self.set_info(format!("View: {}", self.view_mode.label()));
            }
            Action::EqLowUp => {
                self.player.eq().adjust_low(1.0);
                self.set_info("EQ low +1 dB");
            }
            Action::EqLowDown => {
                self.player.eq().adjust_low(-1.0);
                self.set_info("EQ low -1 dB");
            }
            Action::EqMidUp => {
                self.player.eq().adjust_mid(1.0);
                self.set_info("EQ mid +1 dB");
            }
            Action::EqMidDown => {
                self.player.eq().adjust_mid(-1.0);
                self.set_info("EQ mid -1 dB");
            }
            Action::EqHighUp => {
                self.player.eq().adjust_high(1.0);
                self.set_info("EQ high +1 dB");
            }
            Action::EqHighDown => {
                self.player.eq().adjust_high(-1.0);
                self.set_info("EQ high -1 dB");
            }
            Action::OpenUrl => {
                self.url_editing = true;
                self.status =
                    "URL/search — YouTube, ytmsearch:..., Spotify, radio M3U/PLS — Enter/Esc"
                        .into();
            }
            Action::EqPreset => {
                let presets = crate::eq::PRESETS;
                self.eq_preset_idx = (self.eq_preset_idx + 1) % presets.len();
                let (name, state) = presets[self.eq_preset_idx];
                self.player.eq().set(state);
                self.config.playback.eq_preset = name.to_string();
                let _ = self.config.save();
                self.set_info(format!("EQ Preset: 🎚️ {name}"));
            }
            Action::Rescan => self.start_async_scan(),
            Action::TrackInfo => self.show_info = true,
            Action::CycleTheme => self.cycle_theme(),
            Action::VizSensUp => self.adjust_viz_sensitivity(crate::visualizer::SENS_STEP),
            Action::VizSensDown => self.adjust_viz_sensitivity(-crate::visualizer::SENS_STEP),
            Action::UndoQueue => self.undo_queue_action(),
            Action::RecentlyPlayed => {
                if self.view_mode == ViewMode::RecentlyPlayed {
                    self.view_mode = ViewMode::Flat;
                    self.set_info("View: library");
                } else {
                    self.view_mode = ViewMode::RecentlyPlayed;
                    self.set_info(format!("Recently played ({} tracks)", self.history.len()));
                }
                self.library_state
                    .select(if self.visible_library().is_empty() {
                        None
                    } else {
                        Some(0)
                    });
            }
            Action::ShowAudioPanel => {
                self.show_audio_panel = !self.show_audio_panel;
                self.audio_panel_row = 0;
            }
            Action::ReplayGain => {
                self.replaygain_mode = self.replaygain_mode.cycle();
                self.set_info(format!("ReplayGain: {}", self.replaygain_mode.label()));
            }
            Action::CycleVizMode => {
                self.viz_mode = self.viz_mode.cycle();
                self.set_info(format!("Visualizer: {}", self.viz_mode.label()));
            }
            Action::ToggleMini => {
                self.mini_mode = !self.mini_mode;
            }
            Action::LastfmLogin => self.lastfm_login(),
            Action::SelectDevice => self.open_device_selector(),
            Action::EqTuner => {
                self.show_eq_tuner = !self.show_eq_tuner;
                self.eq_tuner_band = 0;
            }
            Action::ToggleFavorite => {
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
            Action::EditTags => self.open_tag_editor(),
            Action::RadioBrowser => {
                self.view_mode = ViewMode::Radio;
                self.focus = Pane::Library;
                self.set_info("📻 Radio Mode — Tab switch pane · Enter play · / search");
            }
            Action::SelfUpdate => self.handle_self_update(),
            Action::ViewLibrary => {
                self.view_mode = ViewMode::Flat;
                self.focus = Pane::Library;
                self.set_info("View: Library (1)");
            }
            Action::ViewQueue => {
                self.focus = Pane::Queue;
                self.set_info("Focus: Queue (2)");
            }
            Action::ViewRadio => {
                self.view_mode = ViewMode::Radio;
                self.focus = Pane::Library;
                self.set_info("📻 Radio Mode (3) — Tab switch pane · Enter play · / search");
            }
            Action::ViewBrowser => {
                self.view_mode = ViewMode::Browser;
                self.focus = Pane::Library;
                self.set_info("View: Folders Browser (4)");
            }
            Action::ShowLyrics => {
                self.show_lyrics = !self.show_lyrics;
                self.lyrics_auto_scroll = true;
                if self.show_lyrics {
                    if let Some(track) = self.player.current().cloned() {
                        if self.lyrics.is_none() {
                            self.spawn_lyrics_fetch(&track);
                        }
                    }
                }
            }
            Action::CommandPalette => {
                self.show_command_palette = true;
                self.command_palette_input.clear();
                self.command_palette_row = 0;
                self.update_command_palette_matches();
            }
            Action::SubsonicBrowser => {
                self.show_subsonic_browser = true;
                self.subsonic_load_tab(self.subsonic_browser_tab);
            }
        }
    }

    pub(crate) fn handle_command_palette_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_command_palette = false;
                self.command_palette_input.clear();
                self.command_palette_matches.clear();
            }
            KeyCode::Down | KeyCode::Tab => {
                if !self.command_palette_matches.is_empty() {
                    self.command_palette_row =
                        (self.command_palette_row + 1) % self.command_palette_matches.len();
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if !self.command_palette_matches.is_empty() {
                    self.command_palette_row = if self.command_palette_row == 0 {
                        self.command_palette_matches.len() - 1
                    } else {
                        self.command_palette_row - 1
                    };
                }
            }
            KeyCode::Enter => {
                if let Some(item) = self
                    .command_palette_matches
                    .get(self.command_palette_row)
                    .cloned()
                {
                    self.show_command_palette = false;
                    self.command_palette_input.clear();
                    self.command_palette_matches.clear();
                    self.execute_palette_action(item.action);
                }
            }
            KeyCode::Backspace => {
                self.command_palette_input.pop();
                self.command_palette_row = 0;
                self.update_command_palette_matches();
            }
            KeyCode::Char(c) => {
                self.command_palette_input.push(c);
                self.command_palette_row = 0;
                self.update_command_palette_matches();
            }
            _ => {}
        }
    }

    pub(crate) fn execute_palette_action(&mut self, action: crate::app::types::PaletteAction) {
        use crate::app::types::PaletteAction;
        match action {
            PaletteAction::Execute(act) => self.run_action(act),
            PaletteAction::SetTheme(name) => self.apply_theme_by_name(&name),
            PaletteAction::SetEqPreset(idx) => self.apply_eq_preset_by_idx(idx),
            PaletteAction::PlayTrack(path) => self.play_track_by_path(&path),
            PaletteAction::SetViewMode(mode) => {
                self.view_mode = mode;
                self.focus = Pane::Library;
                self.set_info(format!("View: {}", mode.label()));
            }
        }
    }

    pub(crate) fn apply_theme_by_name(&mut self, name: &str) {
        match crate::theme::Theme::load(name) {
            Ok(t) => {
                self.theme = t;
                self.config.theme = name.to_string();
                let _ = self.config.save();
                self.set_info(format!("Tema aplicado: 🎨 {name}"));
            }
            Err(e) => self.set_error(format!("Erro ao carregar tema {name}: {e}")),
        }
    }

    pub(crate) fn apply_eq_preset_by_idx(&mut self, idx: usize) {
        let presets = crate::eq::PRESETS;
        if let Some((name, state)) = presets.get(idx) {
            self.eq_preset_idx = idx;
            self.player.eq().set(*state);
            self.config.playback.eq_preset = name.to_string();
            let _ = self.config.save();
            self.set_info(format!("Equalizador: 🎚️ {name}"));
        }
    }

    pub(crate) fn play_track_by_path(&mut self, path: &std::path::Path) {
        if let Some(pos) = self.queue.iter().position(|t| t.path == path) {
            self.queue_index = Some(pos);
            self.queue_state.select(Some(pos));
            self.play_current();
        } else if let Some(track) = self.library.iter().find(|t| t.path == path).cloned() {
            self.queue.push(track);
            let idx = self.queue.len() - 1;
            self.queue_index = Some(idx);
            self.queue_state.select(Some(idx));
            self.play_current();
        }
    }

    pub(crate) fn update_command_palette_matches(&mut self) {
        use crate::app::types::{PaletteCategory, PaletteItem, PaletteAction};

        let raw_query = self.command_palette_input.trim();
        let is_cmd_mode = raw_query.starts_with('>') || raw_query.starts_with(':');
        let query = if is_cmd_mode {
            raw_query[1..].trim()
        } else {
            raw_query
        };

        let mut items: Vec<PaletteItem> = Vec::new();

        // 1. Built-in Commands
        let cmds = [
            ("play", "Play / Pause", "Alternar reprodução e pausa", Action::PlayPause),
            ("next", "Próxima Música", "Avançar para a próxima faixa da fila", Action::Next),
            ("prev", "Música Anterior", "Voltar para a faixa anterior", Action::Prev),
            ("stop", "Parar Reprodução", "Parar áudio e descarregar sink", Action::Stop),
            ("shuffle", "Alternar Shuffle", "Ativar ou desativar modo aleatório", Action::Shuffle),
            ("repeat", "Alternar Repeat", "Ciclar modos de repetição (off/all/one)", Action::Repeat),
            ("mini", "Alternar Mini Player", "Alternar modo compacto com capa de álbum", Action::ToggleMini),
            ("eq", "Equalizador Tuner", "Abrir calibrador de frequências de áudio", Action::EqTuner),
            ("audio", "Painel de Áudio & Compressor", "Ajustar dinâmica e ganho", Action::ShowAudioPanel),
            ("viz", "Ciclar Visualizador FFT", "Mudar modo: spectrum, waveform, vu-meter...", Action::CycleVizMode),
            ("lyrics", "Letras / Karaokê", "Abrir modal de letras sincronizadas (LRC)", Action::ShowLyrics),
            ("radio", "Rádios Online", "Explorar diretório mundial de web rádios", Action::RadioBrowser),
            ("spotify", "Spotify Browser", "Buscar e navegar nas playlists do Spotify", Action::SpotifyBrowser),
            ("subsonic", "Subsonic / Navidrome Cloud", "Streaming pessoal da sua nuvem privada", Action::SubsonicBrowser),
            ("playlists", "Gerenciador de Playlists", "Salvar e carregar arquivos .m3u", Action::LoadPlaylist),
            ("rescan", "Reescanear Biblioteca", "Procurar novos arquivos de música no disco", Action::Rescan),
            ("stats", "Estatísticas de Audição", "Ver artistas e gêneros mais tocados", Action::ShowStats),
            ("lastfm", "Painel Last.fm", "Ver histórico de scrobbling e tops", Action::LastfmPanel),
            ("fav", "Favoritar Música Atual (♥)", "Adicionar/remover dos favoritos", Action::ToggleFavorite),
            ("sleep", "Sleep Timer", "Temporizador de 30min para desligamento", Action::SleepTimer),
            ("tags", "Editor de Tags ID3", "Editar título, artista e álbum do arquivo", Action::EditTags),
            ("help", "Ajuda & Atalhos", "Ver lista completa de atalhos do teclado", Action::Help),
            ("update", "Verificar Atualizações", "Checar nova versão no GitHub", Action::SelfUpdate),
            ("quit", "Sair do Noctune", "Fechar o player", Action::Quit),
        ];

        for (id, title, desc, act) in cmds {
            items.push(PaletteItem {
                id: id.to_string(),
                title: title.to_string(),
                description: desc.to_string(),
                category: PaletteCategory::Command,
                action: PaletteAction::Execute(act),
            });
        }

        // 2. Views
        let views = [
            ("view-library", "Visão: Biblioteca Flat", "Ver lista completa de músicas", ViewMode::Flat),
            ("view-albums", "Visão: Por Álbuns", "Ver biblioteca agrupada por álbum", ViewMode::Albums),
            ("view-smart", "Visão: Playlists Inteligentes", "Mais tocadas, favoritas e recentes", ViewMode::Smart),
            ("view-browser", "Visão: Explorador de Pastas", "Navegar no sistema de arquivos", ViewMode::Browser),
            ("view-radio", "Visão: Hub de Rádios", "Painel dedicado para estações online", ViewMode::Radio),
            ("view-recent", "Visão: Tocadas Recentemente", "Histórico de reprodução", ViewMode::RecentlyPlayed),
        ];
        for (id, title, desc, mode) in views {
            items.push(PaletteItem {
                id: id.to_string(),
                title: title.to_string(),
                description: desc.to_string(),
                category: PaletteCategory::View,
                action: PaletteAction::SetViewMode(mode),
            });
        }

        // 3. Themes
        let available_themes = [
            "default", "catppuccin", "dracula", "nord", "tokyonight",
            "gruvbox", "monokai", "solarized-dark", "cyberpunk", "rose-pine", "synthwave", "amoled"
        ];
        for t in available_themes {
            items.push(PaletteItem {
                id: format!("theme-{t}"),
                title: format!("Tema: {t}"),
                description: "Aplicar tema visual".to_string(),
                category: PaletteCategory::Theme,
                action: PaletteAction::SetTheme(t.to_string()),
            });
        }

        // 4. EQ Presets
        for (i, (name, st)) in crate::eq::PRESETS.iter().enumerate() {
            items.push(PaletteItem {
                id: format!("eq-{name}"),
                title: format!("EQ Preset: {name}"),
                description: format!("Graves {:+.0}dB · Médios {:+.0}dB · Agudos {:+.0}dB", st.low_db(), st.mid_db(), st.high_db()),
                category: PaletteCategory::EqPreset,
                action: PaletteAction::SetEqPreset(i),
            });
        }

        // 5. If query is not command-mode, also search library tracks
        if !is_cmd_mode && !query.is_empty() {
            let q_lower = query.to_lowercase();
            let mut count = 0;
            for t in &self.library {
                if count >= 30 {
                    break;
                }
                if t.title.to_lowercase().contains(&q_lower)
                    || t.artist.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                    || t.album.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                {
                    let dur_str = t.duration.map(format_duration).unwrap_or_else(|| "--:--".into());
                    let desc = format!("{} • {}", t.album.as_deref().unwrap_or("Sem Álbum"), dur_str);
                    items.push(PaletteItem {
                        id: format!("track-{}", t.path.display()),
                        title: t.display(),
                        description: desc,
                        category: PaletteCategory::Track,
                        action: PaletteAction::PlayTrack(t.path.clone()),
                    });
                    count += 1;
                }
            }
        }

        // Filter and sort by fuzzy score
        if query.is_empty() {
            self.command_palette_matches = items;
        } else {
            let mut scored: Vec<(i64, PaletteItem)> = items
                .into_iter()
                .filter_map(|item| {
                    let score_title = fuzzy_score(query, &item.title);
                    let score_desc = fuzzy_score(query, &item.description).map(|s| s / 2);
                    let best = score_title.max(score_desc);
                    best.map(|s| (s, item))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.command_palette_matches = scored.into_iter().map(|(_, item)| item).collect();
        }
    }

    pub(crate) fn handle_audio_panel_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('e') => {
                self.show_audio_panel = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.audio_panel_row = self.audio_panel_row.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.audio_panel_row + 1 < Self::AUDIO_PANEL_ROWS =>
            {
                self.audio_panel_row += 1;
            }
            KeyCode::Left => self.audio_panel_adjust(-1),
            KeyCode::Right => self.audio_panel_adjust(1),
            _ => {}
        }
    }

    pub(crate) fn handle_tag_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_tag_editor = false;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.tag_editor_row = (self.tag_editor_row + 1) % 5;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.tag_editor_row = if self.tag_editor_row == 0 {
                    4
                } else {
                    self.tag_editor_row - 1
                };
            }
            KeyCode::Enter => {
                self.save_tag_editor();
                self.show_tag_editor = false;
            }
            KeyCode::Backspace => {
                self.tag_editor_fields[self.tag_editor_row].pop();
            }
            KeyCode::Char(c) => {
                self.tag_editor_fields[self.tag_editor_row].push(c);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_radio_browser_key(&mut self, key: KeyEvent) {
        if self.radio_search_editing {
            match key.code {
                KeyCode::Esc => {
                    self.radio_search_editing = false;
                }
                KeyCode::Enter => {
                    self.radio_search_editing = false;
                    self.trigger_radio_search();
                }
                KeyCode::Backspace => {
                    self.radio_search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.radio_search_query.push(c);
                }
                _ => {}
            }
            return;
        }

        let list_len = match self.radio_tab {
            crate::radio_browser::RadioTab::Curated => self.radio_curated_list.len(),
            crate::radio_browser::RadioTab::Search => self.radio_search_results.len(),
        };

        match key.code {
            KeyCode::Esc | KeyCode::Char('K') => {
                self.show_radio_browser = false;
                self.radio_search_editing = false;
            }
            KeyCode::Tab => {
                self.radio_tab = self.radio_tab.cycle();
                self.radio_row = 0;
            }
            KeyCode::Char('/') => {
                self.radio_tab = crate::radio_browser::RadioTab::Search;
                self.radio_search_editing = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.radio_row = self.radio_row.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if list_len > 0 && self.radio_row + 1 < list_len {
                    self.radio_row += 1;
                }
            }
            KeyCode::Enter => {
                let station = match self.radio_tab {
                    crate::radio_browser::RadioTab::Curated => {
                        self.radio_curated_list.get(self.radio_row).cloned()
                    }
                    crate::radio_browser::RadioTab::Search => {
                        self.radio_search_results.get(self.radio_row).cloned()
                    }
                };
                if let Some(st) = station {
                    self.play_radio_station(&st, false);
                }
            }
            KeyCode::Char('a') => {
                let station = match self.radio_tab {
                    crate::radio_browser::RadioTab::Curated => {
                        self.radio_curated_list.get(self.radio_row).cloned()
                    }
                    crate::radio_browser::RadioTab::Search => {
                        self.radio_search_results.get(self.radio_row).cloned()
                    }
                };
                if let Some(st) = station {
                    self.play_radio_station(&st, true);
                }
            }
            KeyCode::Char('+') | KeyCode::Char('N') | KeyCode::Char('n') => {
                self.show_radio_custom_modal = true;
                self.radio_custom_fields = [String::new(), String::new(), String::new()];
                self.radio_custom_field_idx = 0;
            }
            KeyCode::Char('f') => {
                let station = match self.radio_tab {
                    crate::radio_browser::RadioTab::Curated => {
                        self.radio_curated_list.get(self.radio_row).cloned()
                    }
                    crate::radio_browser::RadioTab::Search => {
                        self.radio_search_results.get(self.radio_row).cloned()
                    }
                };
                if let Some(st) = station {
                    let p = std::path::PathBuf::from(&st.url);
                    let fav = self.ratings.toggle_favorite(&p);
                    self.set_info(if fav {
                        format!("Rádio favoritada: {} ♥", st.name)
                    } else {
                        format!("Rádio removida dos favoritos: {}", st.name)
                    });
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_radio_custom_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_radio_custom_modal = false;
                self.radio_custom_fields = [String::new(), String::new(), String::new()];
                self.radio_custom_field_idx = 0;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.radio_custom_field_idx = (self.radio_custom_field_idx + 1) % 3;
            }
            KeyCode::BackTab | KeyCode::Up => {
                if self.radio_custom_field_idx == 0 {
                    self.radio_custom_field_idx = 2;
                } else {
                    self.radio_custom_field_idx -= 1;
                }
            }
            KeyCode::Enter => {
                if self.radio_custom_field_idx == 0 && self.radio_custom_fields[1].is_empty() {
                    self.radio_custom_field_idx = 1;
                    return;
                }
                self.save_custom_radio_station();
            }
            KeyCode::Backspace => {
                self.radio_custom_fields[self.radio_custom_field_idx].pop();
            }
            KeyCode::Char(c) => {
                self.radio_custom_fields[self.radio_custom_field_idx].push(c);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_device_selector_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('D') => {
                self.show_device_selector = false;
            }
            KeyCode::Up | KeyCode::Char('k') if self.device_selector_row > 0 => {
                self.device_selector_row -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.device_selector_row + 1 < self.device_list.len() =>
            {
                self.device_selector_row += 1;
            }
            KeyCode::Enter => {
                self.show_device_selector = false;
                if let Some(name) = self.device_list.get(self.device_selector_row).cloned() {
                    match self.player.switch_device(&name) {
                        Ok(_) => self.set_info(format!("Output device: {name}")),
                        Err(e) => self.set_info(format!("Device error: {e}")),
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_eq_tuner_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.eq_preset_name_editing = true;
            self.eq_preset_name_input.clear();
            return;
        }

        let eq = self.player.eq();
        match key.code {
            KeyCode::Esc | KeyCode::Char('E') => {
                self.show_eq_tuner = false;
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                self.eq_tuner_band = if self.eq_tuner_band == 0 {
                    crate::eq::NUM_BANDS - 1
                } else {
                    self.eq_tuner_band - 1
                };
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.eq_tuner_band = (self.eq_tuner_band + 1) % crate::eq::NUM_BANDS;
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('+') | KeyCode::Char('=') => {
                eq.adjust_band(self.eq_tuner_band, 1.0);
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('-') => {
                eq.adjust_band(self.eq_tuner_band, -1.0);
            }
            KeyCode::Char('r') => {
                eq.set(crate::eq::EqState::default());
                self.set_info("Equalizador: Flat (0 dB)");
            }
            KeyCode::Char('0') => {
                let snap = eq.snapshot();
                let builtins = crate::eq::PRESETS;
                let all: Vec<(&str, crate::eq::EqState)> = builtins
                    .iter()
                    .map(|(n, s)| (*n, *s))
                    .chain(self.custom_eq_presets.iter().map(|p| {
                        (p.name.as_str(), p.to_eq_state())
                    }))
                    .collect();
                let next = all
                    .iter()
                    .position(|(_, s)| {
                        s.bands.iter().zip(snap.bands.iter()).all(|(a, b)| (a - b).abs() < 0.1)
                    })
                    .map(|i| (i + 1) % all.len())
                    .unwrap_or(0);
                eq.set(all[next].1);
                self.set_info(format!("EQ Preset: 🎚️ {}", all[next].0));
            }
            _ => {}
        }
    }

    pub(crate) fn handle_playlist_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_playlist_browser = false;
                self.playlist_browser_delete_confirm = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.playlist_browser_row = self.playlist_browser_row.saturating_sub(1);
                self.playlist_browser_delete_confirm = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.playlist_browser_entries.len().saturating_sub(1);
                if self.playlist_browser_row < max {
                    self.playlist_browser_row += 1;
                }
                self.playlist_browser_delete_confirm = None;
            }
            KeyCode::Enter => {
                self.load_playlist_at_row(false);
            }
            KeyCode::Char('a') => {
                self.load_playlist_at_row(true);
            }
            KeyCode::Char('D') => {
                let row = self.playlist_browser_row;
                if self.playlist_browser_delete_confirm == Some(row) {
                    if let Some(entry) = self.playlist_browser_entries.get(row).cloned() {
                        if std::fs::remove_file(&entry.path).is_ok() {
                            self.playlist_browser_entries.remove(row);
                            self.playlist_browser_row =
                                row.min(self.playlist_browser_entries.len().saturating_sub(1));
                            self.set_info(format!("Deleted: {}", entry.name));
                            if self.active_playlist_name.as_deref() == Some(&entry.name) {
                                self.active_playlist_name = None;
                            }
                        }
                    }
                    self.playlist_browser_delete_confirm = None;
                    if self.playlist_browser_entries.is_empty() {
                        self.show_playlist_browser = false;
                    }
                } else {
                    self.playlist_browser_delete_confirm = Some(row);
                    self.set_info("Press Shift+D again to confirm deletion.");
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_profile_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.show_profile_browser = false;
            }
            KeyCode::Up | KeyCode::Char('k') if self.profile_browser_row > 0 => {
                self.profile_browser_row -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.profile_browser_row + 1 < self.profiles.len() =>
            {
                self.profile_browser_row += 1;
            }
            KeyCode::Enter => {
                let row = self.profile_browser_row;
                self.show_profile_browser = false;
                self.apply_profile(row);
            }
            KeyCode::Char('n') => {
                self.show_profile_browser = false;
                self.profile_name_editing = true;
                self.profile_name_input.clear();
                self.set_info("Profile name (Enter to save, Esc to cancel):");
            }
            KeyCode::Char('D') if self.profile_browser_row < self.profiles.len() => {
                let removed = self.profiles.remove(self.profile_browser_row);
                if self.profile_browser_row > 0 {
                    self.profile_browser_row -= 1;
                }
                let store = crate::config::Profiles {
                    profiles: self.profiles.clone(),
                };
                let _ = store.save();
                self.set_info(format!("Profile '{}' deleted.", removed.name));
            }
            _ => {}
        }
    }

    pub(crate) fn handle_spotify_browser_key(&mut self, key: KeyEvent) {
        if self.spotify_browser_query_editing {
            match key.code {
                KeyCode::Esc => {
                    self.spotify_browser_query_editing = false;
                    if self.spotify_browser_query.is_empty() {
                        self.show_spotify_browser = false;
                    }
                }
                KeyCode::Enter => {
                    self.spotify_browser_query_editing = false;
                    self.spotify_browser_tab = SpotifyTab::Search;
                    self.spotify_search();
                }
                KeyCode::Backspace => {
                    self.spotify_browser_query.pop();
                }
                KeyCode::Char(c) => {
                    self.spotify_browser_query.push(c);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.show_spotify_browser = false;
            }
            KeyCode::Tab => {
                self.spotify_browser_tab = match self.spotify_browser_tab {
                    SpotifyTab::Search => {
                        if self.spotify_my_playlists.is_empty() {
                            self.spotify_load_my_playlists();
                        }
                        SpotifyTab::MyPlaylists
                    }
                    SpotifyTab::MyPlaylists => {
                        self.spotify_load_liked();
                        SpotifyTab::LikedSongs
                    }
                    SpotifyTab::LikedSongs => SpotifyTab::Search,
                };
                self.spotify_browser_row = 0;
                self.spotify_playlist_row = 0;
            }
            KeyCode::Char('/') | KeyCode::Char('s') => {
                self.spotify_browser_tab = SpotifyTab::Search;
                self.spotify_browser_query_editing = true;
            }
            KeyCode::Up | KeyCode::Char('k') => match self.spotify_browser_tab {
                SpotifyTab::MyPlaylists => {
                    if self.spotify_playlist_row > 0 {
                        self.spotify_playlist_row -= 1;
                    }
                }
                _ => {
                    if self.spotify_browser_row > 0 {
                        self.spotify_browser_row -= 1;
                    }
                }
            },
            KeyCode::Down | KeyCode::Char('j') => match self.spotify_browser_tab {
                SpotifyTab::MyPlaylists => {
                    if self.spotify_playlist_row + 1 < self.spotify_my_playlists.len() {
                        self.spotify_playlist_row += 1;
                    }
                }
                _ => {
                    if self.spotify_browser_row + 1 < self.spotify_browser_results.len() {
                        self.spotify_browser_row += 1;
                    }
                }
            },
            KeyCode::Enter => {
                self.show_spotify_browser = false;
                match self.spotify_browser_tab {
                    SpotifyTab::MyPlaylists => {
                        if let Some((id, name, _)) = self
                            .spotify_my_playlists
                            .get(self.spotify_playlist_row)
                            .cloned()
                        {
                            self.spotify_load_playlist(id, name);
                        }
                    }
                    _ => {
                        if let Some(t) = self
                            .spotify_browser_results
                            .get(self.spotify_browser_row)
                            .cloned()
                        {
                            self.queue.push(t.clone());
                            self.queue_index = Some(self.queue.len() - 1);
                            self.queue_state.select(self.queue_index);
                            self.play_current();
                        }
                    }
                }
            }
            KeyCode::Char('a') => match self.spotify_browser_tab {
                SpotifyTab::MyPlaylists => {
                    if let Some((id, name, _)) = self
                        .spotify_my_playlists
                        .get(self.spotify_playlist_row)
                        .cloned()
                    {
                        self.show_spotify_browser = false;
                        self.spotify_load_playlist(id, name);
                    }
                }
                _ => {
                    if let Some(t) = self
                        .spotify_browser_results
                        .get(self.spotify_browser_row)
                        .cloned()
                    {
                        self.queue.push(t);
                        self.set_info("Added to queue.");
                    }
                }
            },
            _ => {}
        }
    }

    pub(crate) fn handle_subsonic_browser_key(&mut self, key: KeyEvent) {
        use crate::app::types::SubsonicTab;
        if self.subsonic_browser_query_editing {
            match key.code {
                KeyCode::Esc => {
                    self.subsonic_browser_query_editing = false;
                    if self.subsonic_browser_query.is_empty()
                        && self.subsonic_browser_results.is_empty()
                    {
                        self.show_subsonic_browser = false;
                    }
                }
                KeyCode::Enter => {
                    self.subsonic_browser_query_editing = false;
                    self.subsonic_browser_tab = SubsonicTab::Search;
                    self.subsonic_search();
                }
                KeyCode::Backspace => {
                    self.subsonic_browser_query.pop();
                }
                KeyCode::Char(c) => {
                    self.subsonic_browser_query.push(c);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.show_subsonic_browser = false;
            }
            KeyCode::Tab => {
                let next = match self.subsonic_browser_tab {
                    SubsonicTab::Search => SubsonicTab::RecentAlbums,
                    SubsonicTab::RecentAlbums => SubsonicTab::Playlists,
                    SubsonicTab::Playlists => SubsonicTab::Random,
                    SubsonicTab::Random => SubsonicTab::Search,
                };
                self.subsonic_load_tab(next);
            }
            KeyCode::BackTab => {
                let prev = match self.subsonic_browser_tab {
                    SubsonicTab::Search => SubsonicTab::Random,
                    SubsonicTab::RecentAlbums => SubsonicTab::Search,
                    SubsonicTab::Playlists => SubsonicTab::RecentAlbums,
                    SubsonicTab::Random => SubsonicTab::Playlists,
                };
                self.subsonic_load_tab(prev);
            }
            KeyCode::Char('/') | KeyCode::Char('s') => {
                self.subsonic_browser_tab = SubsonicTab::Search;
                self.subsonic_browser_query_editing = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.subsonic_browser_row > 0 {
                    self.subsonic_browser_row -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max_len = match self.subsonic_browser_tab {
                    SubsonicTab::Search | SubsonicTab::Random => {
                        self.subsonic_browser_results.len()
                    }
                    SubsonicTab::RecentAlbums => self.subsonic_browser_albums.len(),
                    SubsonicTab::Playlists => self.subsonic_browser_playlists.len(),
                };
                if self.subsonic_browser_row + 1 < max_len {
                    self.subsonic_browser_row += 1;
                }
            }
            KeyCode::Enter => match self.subsonic_browser_tab {
                SubsonicTab::Search | SubsonicTab::Random => {
                    if let Some(track) = self
                        .subsonic_browser_results
                        .get(self.subsonic_browser_row)
                        .cloned()
                    {
                        self.show_subsonic_browser = false;
                        self.queue.push(track);
                        let idx = self.queue.len() - 1;
                        self.queue_index = Some(idx);
                        self.queue_state.select(Some(idx));
                        self.play_current();
                    }
                }
                SubsonicTab::RecentAlbums => {
                    if let Some(album) = self
                        .subsonic_browser_albums
                        .get(self.subsonic_browser_row)
                        .cloned()
                    {
                        self.subsonic_load_album_tracks(&album.id);
                        self.subsonic_browser_tab = SubsonicTab::Search;
                    }
                }
                SubsonicTab::Playlists => {
                    if let Some(pl) = self
                        .subsonic_browser_playlists
                        .get(self.subsonic_browser_row)
                        .cloned()
                    {
                        self.subsonic_load_playlist_tracks(&pl.id);
                        self.subsonic_browser_tab = SubsonicTab::Search;
                    }
                }
            },
            KeyCode::Char('a') => match self.subsonic_browser_tab {
                SubsonicTab::Search | SubsonicTab::Random => {
                    if let Some(track) = self
                        .subsonic_browser_results
                        .get(self.subsonic_browser_row)
                        .cloned()
                    {
                        self.queue.push(track);
                        self.set_info("Faixa adicionada à fila.");
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub(crate) fn handle_lyrics_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('y') | KeyCode::Char('q') => {
                self.show_lyrics = false;
            }
            KeyCode::Char(' ') => {
                self.player.toggle();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.lyrics_scroll = self.lyrics_scroll.saturating_sub(1);
                self.lyrics_auto_scroll = false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max_len = self.lyrics.as_ref().map(|l| l.lines.len()).unwrap_or(0);
                if max_len > 0 && self.lyrics_scroll + 1 < max_len {
                    self.lyrics_scroll += 1;
                }
                self.lyrics_auto_scroll = false;
            }
            KeyCode::Char('c') | KeyCode::Char('a') => {
                self.lyrics_auto_scroll = true;
                self.set_info("Karaoke: auto-scroll ativado");
            }
            KeyCode::Char('r') => {
                if let Some(track) = self.player.current().cloned() {
                    self.spawn_lyrics_fetch(&track);
                    self.set_info("Buscando letras no LRCLIB…");
                }
            }
            KeyCode::Enter => {
                let target_dur = self
                    .lyrics
                    .as_ref()
                    .and_then(|l| l.lines.get(self.lyrics_scroll).map(|line| line.at));
                if let Some(dur) = target_dur {
                    self.seek_to_async(dur);
                    self.lyrics_auto_scroll = true;
                    self.set_info(format!(
                        "Letra: saltou para {}",
                        format_duration(dur)
                    ));
                }
            }
            _ => {}
        }
    }

    pub(crate) fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.move_selection(-1),
            MouseEventKind::ScrollDown => self.move_selection(1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_click(m.column, m.row);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.handle_drag(m.column, m.row);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.last_drag_seek = None;
            }
            MouseEventKind::Moved => {
                let prog = self.layout.progress;
                if m.row >= prog.y && m.row < prog.y + prog.height && prog.width > 0 {
                    self.hover_x = Some(m.column);
                } else {
                    self.hover_x = None;
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_drag(&mut self, x: u16, y: u16) {
        let prog = self.layout.progress;
        if !rect_contains(prog, x, y) || prog.width == 0 {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.last_drag_seek {
            if now.duration_since(last) < Duration::from_millis(50) {
                return;
            }
        }
        let frac = (x.saturating_sub(prog.x)) as f32 / prog.width as f32;
        if let Err(e) = self.seek_fraction_async(frac) {
            self.set_info(format!("Seek error: {e}"));
        }
        self.last_drag_seek = Some(now);
    }

    pub(crate) fn handle_click(&mut self, x: u16, y: u16) {
        let lib = self.layout.library;
        let q = self.layout.queue;
        let prog = self.layout.progress;

        if rect_contains(lib, x, y) {
            self.focus = Pane::Library;
            let row = (y - lib.y) as usize;
            let rows = self.library_rows();
            if let Some(item) = rows.get(row) {
                if matches!(item, LibraryRow::Track(_)) {
                    self.library_state.select(Some(row));
                    self.activate_selection();
                }
            }
            return;
        }
        if rect_contains(q, x, y) {
            self.focus = Pane::Queue;
            let row = (y - q.y) as usize;
            if row < self.queue.len() {
                self.queue_state.select(Some(row));
                self.queue_index = Some(row);
                self.play_current();
            }
            return;
        }
        if rect_contains(prog, x, y) && prog.width > 0 {
            let frac = (x.saturating_sub(prog.x)) as f32 / prog.width as f32;
            if let Err(e) = self.seek_fraction_async(frac) {
                self.set_info(format!("Seek error: {e}"));
            }
        }
    }
}

fn fuzzy_score(pattern: &str, text: &str) -> Option<i64> {
    let p_lower = pattern.to_lowercase();
    let t_lower = text.to_lowercase();

    if let Some(pos) = t_lower.find(&p_lower) {
        return Some(1000 - (pos as i64 * 10) - (t_lower.len() as i64));
    }

    let mut score = 0i64;
    let mut t_chars = t_lower.char_indices();
    let mut last_idx = 0;
    for pc in p_lower.chars() {
        loop {
            match t_chars.next() {
                Some((idx, tc)) if tc == pc => {
                    if idx == last_idx {
                        score += 40;
                    } else {
                        score += 10;
                    }
                    last_idx = idx + 1;
                    break;
                }
                Some(_) => {}
                None => return None,
            }
        }
    }
    Some(score - (t_lower.len() as i64))
}

