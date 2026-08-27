use anyhow::{anyhow, Result};
use mlua::{Function, Lua, LuaSerdeExt, RegistryKey, Table, Value};
use parking_lot::Mutex;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::audio::Track;

#[derive(Clone)]
pub struct PluginCommandInfo {
    pub id: String,
    pub title: String,
    pub description: String,
}

pub struct PluginEngine {
    lua: Lua,
    handlers: Vec<(String, RegistryKey)>,
    commands: Vec<(PluginCommandInfo, RegistryKey)>,
    messages: Arc<Mutex<Vec<String>>>,
    actions: Arc<Mutex<Vec<crate::keybinds::Action>>>,
    current_track: Arc<Mutex<Option<Track>>>,
    volume: Arc<Mutex<f32>>,
}

impl PluginEngine {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let actions = Arc::new(Mutex::new(Vec::new()));
        let current_track = Arc::new(Mutex::new(None));
        let volume = Arc::new(Mutex::new(0.7f32));

        let engine = Self {
            lua,
            handlers: Vec::new(),
            commands: Vec::new(),
            messages,
            actions,
            current_track,
            volume,
        };

        engine.setup_global_api()?;
        Ok(engine)
    }

    fn setup_global_api(&self) -> Result<()> {
        let globals = self.lua.globals();
        let noctune_table = self.lua.create_table()?;

        // noctune.notify(msg)
        let msgs = self.messages.clone();
        let notify_fn = self.lua.create_function(move |_, msg: String| {
            msgs.lock().push(msg);
            Ok(())
        })?;
        noctune_table.set("notify", notify_fn)?;

        // noctune.play_pause()
        let acts = self.actions.clone();
        let play_pause_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::PlayPause);
            Ok(())
        })?;
        noctune_table.set("play_pause", play_pause_fn)?;

        // noctune.next()
        let acts = self.actions.clone();
        let next_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::Next);
            Ok(())
        })?;
        noctune_table.set("next", next_fn)?;

        // noctune.prev()
        let acts = self.actions.clone();
        let prev_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::Prev);
            Ok(())
        })?;
        noctune_table.set("prev", prev_fn)?;

        // noctune.stop()
        let acts = self.actions.clone();
        let stop_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::Stop);
            Ok(())
        })?;
        noctune_table.set("stop", stop_fn)?;

        // noctune.get_track()
        let cur = self.current_track.clone();
        let get_track_fn = self.lua.create_function(move |lua, ()| {
            let track_opt = cur.lock();
            if let Some(t) = track_opt.as_ref() {
                let tab = lua.create_table()?;
                tab.set("title", t.title.clone())?;
                tab.set("artist", t.artist.clone().unwrap_or_default())?;
                tab.set("album", t.album.clone().unwrap_or_default())?;
                tab.set("duration", t.duration.map(|d| d.as_secs()).unwrap_or(0))?;
                tab.set("path", t.path.to_string_lossy().to_string())?;
                Ok(Value::Table(tab))
            } else {
                Ok(Value::Nil)
            }
        })?;
        noctune_table.set("get_track", get_track_fn)?;

        // noctune.get_volume()
        let vol = self.volume.clone();
        let get_vol_fn = self.lua.create_function(move |_, ()| {
            Ok(*vol.lock())
        })?;
        noctune_table.set("get_volume", get_vol_fn)?;

        // noctune.volume_up()
        let acts = self.actions.clone();
        let vol_up_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::VolumeUp);
            Ok(())
        })?;
        noctune_table.set("volume_up", vol_up_fn)?;

        // noctune.volume_down()
        let acts = self.actions.clone();
        let vol_down_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::VolumeDown);
            Ok(())
        })?;
        noctune_table.set("volume_down", vol_down_fn)?;

        // noctune.toggle_mini()
        let acts = self.actions.clone();
        let toggle_mini_fn = self.lua.create_function(move |_, ()| {
            acts.lock().push(crate::keybinds::Action::ToggleMini);
            Ok(())
        })?;
        noctune_table.set("toggle_mini", toggle_mini_fn)?;

        globals.set("noctune", noctune_table)?;
        Ok(())
    }

    pub fn set_state(&self, track: Option<&Track>, volume: f32) {
        *self.current_track.lock() = track.cloned();
        *self.volume.lock() = volume;
    }

    pub fn load_plugin_file(&mut self, path: &Path) -> Result<String> {
        let code = std::fs::read_to_string(path)?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin")
            .to_string();

        // Environment for this plugin
        let globals = self.lua.globals();
        let noctune_table: Table = globals.get("noctune")?;

        // Helper closures to register events and commands during script execution
        let chunk = self.lua.load(&code).set_name(&name);
        chunk.exec()?;

        // If the script returned or defined hooks, bind them:
        // 1. Hook function: hook(event, callback)
        if let Ok(hooks) = globals.get::<_, Table>("hooks") {
            for pair in hooks.pairs::<String, Function>() {
                if let Ok((event, func)) = pair {
                    let key = self.lua.create_registry_value(func)?;
                    self.handlers.push((event, key));
                }
            }
        }

        // 2. Commands table: commands = { { id = "...", title = "...", desc = "...", callback = fn } }
        if let Ok(cmds) = globals.get::<_, Table>("commands") {
            for val in cmds.sequence_values::<Table>() {
                if let Ok(cmd_tab) = val {
                    let id: String = cmd_tab.get("id").unwrap_or_default();
                    let title: String = cmd_tab.get("title").unwrap_or_default();
                    let description: String = cmd_tab.get("description").unwrap_or_default();
                    if let Ok(cb) = cmd_tab.get::<_, Function>("callback") {
                        let key = self.lua.create_registry_value(cb)?;
                        self.commands.push((
                            PluginCommandInfo {
                                id,
                                title,
                                description,
                            },
                            key,
                        ));
                    }
                }
            }
        }

        Ok(name)
    }

    pub fn load_plugins_dir(&mut self, dir: &Path) -> Vec<String> {
        let mut loaded = Vec::new();
        if !dir.exists() {
            let _ = std::fs::create_dir_all(dir);
            return loaded;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return loaded,
        };

        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("lua") {
                match self.load_plugin_file(&p) {
                    Ok(name) => loaded.push(name),
                    Err(e) => {
                        tracing::warn!(target: "plugins", "Erro ao carregar plugin {}: {e}", p.display());
                    }
                }
            } else if p.is_dir() {
                let init_lua = p.join("init.lua");
                if init_lua.exists() {
                    match self.load_plugin_file(&init_lua) {
                        Ok(name) => loaded.push(name),
                        Err(e) => {
                            tracing::warn!(target: "plugins", "Erro ao carregar plugin {}: {e}", init_lua.display());
                        }
                    }
                }
            }
        }
        loaded
    }

    pub fn trigger(&self, event: &str) {
        for (ev, key) in &self.handlers {
            if ev == event {
                if let Ok(func) = self.lua.registry_value::<Function>(key) {
                    if let Err(e) = func.call::<_, ()>(()) {
                        tracing::warn!(target: "plugins", "Erro no hook '{event}': {e}");
                    }
                }
            }
        }
    }

    pub fn trigger_track_start(&self, track: &Track) {
        *self.current_track.lock() = Some(track.clone());
        for (ev, key) in &self.handlers {
            if ev == "track_start" {
                if let Ok(func) = self.lua.registry_value::<Function>(key) {
                    let tab = match self.lua.create_table() {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let _ = tab.set("title", track.title.clone());
                    let _ = tab.set("artist", track.artist.clone().unwrap_or_default());
                    let _ = tab.set("album", track.album.clone().unwrap_or_default());
                    let _ = tab.set("duration", track.duration.map(|d| d.as_secs()).unwrap_or(0));
                    let _ = tab.set("path", track.path.to_string_lossy().to_string());
                    if let Err(e) = func.call::<_, ()>(tab) {
                        tracing::warn!(target: "plugins", "Erro no hook 'track_start': {e}");
                    }
                }
            }
        }
    }

    pub fn run_command(&self, cmd_id: &str) -> Result<()> {
        for (info, key) in &self.commands {
            if info.id == cmd_id {
                let func = self.lua.registry_value::<Function>(key)?;
                func.call::<_, ()>(())?;
                return Ok(());
            }
        }
        Err(anyhow!("Comando de plugin '{cmd_id}' não encontrado"))
    }

    pub fn commands(&self) -> Vec<PluginCommandInfo> {
        self.commands.iter().map(|(info, _)| info.clone()).collect()
    }

    pub fn drain_messages(&self) -> Vec<String> {
        let mut msgs = self.messages.lock();
        std::mem::take(&mut *msgs)
    }

    pub fn drain_actions(&self) -> Vec<crate::keybinds::Action> {
        let mut acts = self.actions.lock();
        std::mem::take(&mut *acts)
    }
}
