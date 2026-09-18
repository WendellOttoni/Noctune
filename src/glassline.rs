//! Optional Glassline integration. The worker owns all named-pipe I/O so a
//! missing or slow Glassline instance can never block playback or the TUI.

use std::{path::PathBuf, time::Duration};

#[derive(Debug, Clone, Copy)]
pub enum Command {
    Previous,
    TogglePlayback,
    Next,
    ShowNoctune,
}

#[derive(Debug, Clone)]
pub struct PlaybackSnapshot {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork_path: Option<PathBuf>,
    pub duration: Duration,
    pub position: Duration,
    pub playing: bool,
}

#[cfg(target_os = "windows")]
pub fn show_noctune() -> bool {
    use windows_sys::Win32::{
        System::Console::GetConsoleWindow,
        UI::WindowsAndMessaging::SetForegroundWindow,
    };
    unsafe {
        let window = GetConsoleWindow();
        !window.is_null() && SetForegroundWindow(window) != 0
    }
}

#[cfg(not(target_os = "windows"))]
pub fn show_noctune() -> bool {
    true
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{Command, PlaybackSnapshot};
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        fs::{File, OpenOptions},
        io::{self, Read, Write},
        path::PathBuf,
        sync::{mpsc, Arc, Mutex},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const VERSION: u8 = 1;
    const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

    #[derive(Clone, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct WireState {
        track_id: String,
        title: String,
        artist: String,
        album: String,
        artwork_path: Option<PathBuf>,
        duration_ms: u64,
        position_ms: u64,
        sent_at_unix_ms: u64,
        playback: &'static str,
    }

    #[derive(Deserialize)]
    struct IncomingEnvelope {
        version: u8,
        #[serde(rename = "type")]
        message_type: String,
        payload: IncomingCommand,
    }

    #[derive(Deserialize)]
    struct IncomingCommand {
        id: String,
        name: String,
    }

    pub struct GlasslineBridge {
        state: Arc<Mutex<Option<PlaybackSnapshot>>>,
        command_rx: mpsc::Receiver<(String, Command)>,
        result_tx: mpsc::Sender<(String, bool)>,
    }

    impl GlasslineBridge {
        pub fn start() -> Self {
            let state = Arc::new(Mutex::new(None));
            let (command_tx, command_rx) = mpsc::channel();
            let (result_tx, result_rx) = mpsc::channel();
            let worker_state = Arc::clone(&state);
            thread::Builder::new()
                .name("glassline-ipc".into())
                .spawn(move || worker(worker_state, command_tx, result_rx))
                .ok();
            Self {
                state,
                command_rx,
                result_tx,
            }
        }

        pub fn publish(&self, snapshot: Option<PlaybackSnapshot>) {
            if let Ok(mut state) = self.state.lock() {
                *state = snapshot;
            }
        }

        pub fn drain_commands(&self) -> Vec<(String, Command)> {
            self.command_rx.try_iter().collect()
        }

        pub fn complete_command(&self, id: String, success: bool) {
            let _ = self.result_tx.send((id, success));
        }
    }

    fn worker(
        state: Arc<Mutex<Option<PlaybackSnapshot>>>,
        command_tx: mpsc::Sender<(String, Command)>,
        result_rx: mpsc::Receiver<(String, bool)>,
    ) {
        loop {
            match connect() {
                Ok(stream) => {
                    if let Err(error) = run_connection(stream, &state, &command_tx, &result_rx) {
                        tracing::debug!(target: "glassline", "connection ended: {error}");
                    }
                }
                Err(error) => {
                    tracing::trace!(target: "glassline", "not available: {error}");
                }
            }
            thread::sleep(Duration::from_secs(2));
        }
    }

    fn connect() -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!(r"\\.\pipe\{}", pipe_name()))
    }

    fn pipe_name() -> String {
        let domain = std::env::var("USERDOMAIN").unwrap_or_default();
        let user = std::env::var("USERNAME").unwrap_or_default();
        pipe_name_for(&domain, &user)
    }

    fn pipe_name_for(domain: &str, user: &str) -> String {
        let digest = Sha256::digest(format!("{domain}\\{user}").as_bytes());
        let suffix = digest[..8]
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        format!("glassline-noctune-v1-{suffix}")
    }

    fn run_connection(
        stream: File,
        state: &Arc<Mutex<Option<PlaybackSnapshot>>>,
        command_tx: &mpsc::Sender<(String, Command)>,
        result_rx: &mpsc::Receiver<(String, bool)>,
    ) -> io::Result<()> {
        while result_rx.try_recv().is_ok() {}

        let mut reader = stream.try_clone()?;
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let reader_connected = Arc::clone(&connected);
        let reader_commands = command_tx.clone();
        let reader_thread = thread::spawn(move || {
            while reader_connected.load(std::sync::atomic::Ordering::Relaxed) {
                match read_command(&mut reader) {
                    Ok(Some(command)) => {
                        if reader_commands.send(command).is_err() {
                            break;
                        }
                    }
                    Ok(None) | Err(_) => break,
                }
            }
            reader_connected.store(false, std::sync::atomic::Ordering::Relaxed);
        });

        let mut writer = stream;
        let mut last_state: Option<WireState> = None;
        let mut last_anchor = Instant::now() - Duration::from_secs(2);
        write_state_message(&mut writer, "snapshot", snapshot(state))?;

        while connected.load(std::sync::atomic::Ordering::Relaxed) {
            let current = snapshot(state);
            if track_changed(last_state.as_ref(), &current) {
                write_state_message(&mut writer, "track_changed", current.clone())?;
                last_anchor = Instant::now();
            } else if playback_changed(last_state.as_ref(), &current) {
                write_envelope(
                    &mut writer,
                    "playback_changed",
                    json!({ "playback": current.playback }),
                )?;
            }

            let paused_position_changed = current.playback != "playing"
                && last_state
                    .as_ref()
                    .is_some_and(|value| value.position_ms != current.position_ms);
            if paused_position_changed
                || (current.playback == "playing"
                    && last_anchor.elapsed() >= Duration::from_secs(1))
            {
                write_envelope(
                    &mut writer,
                    "position_anchor",
                    json!({
                        "positionMs": current.position_ms,
                        "sentAtUnixMs": current.sent_at_unix_ms,
                    }),
                )?;
                last_anchor = Instant::now();
            }

            last_state = Some(current);
            for (id, success) in result_rx.try_iter() {
                write_envelope(
                    &mut writer,
                    "command_result",
                    json!({
                        "id": id,
                        "success": success,
                        "error": if success { Value::Null } else { json!("command failed") },
                    }),
                )?;
            }
            thread::sleep(Duration::from_millis(100));
        }

        connected.store(false, std::sync::atomic::Ordering::Relaxed);
        drop(writer);
        let _ = reader_thread.join();
        Ok(())
    }

    fn snapshot(state: &Arc<Mutex<Option<PlaybackSnapshot>>>) -> WireState {
        let state = state.lock().ok().and_then(|value| value.clone());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        match state {
            Some(value) => WireState {
                track_id: value.track_id,
                title: value.title,
                artist: value.artist,
                album: value.album,
                artwork_path: value.artwork_path,
                duration_ms: value.duration.as_millis() as u64,
                position_ms: value.position.as_millis() as u64,
                sent_at_unix_ms: now,
                playback: if value.playing { "playing" } else { "paused" },
            },
            None => WireState {
                track_id: String::new(),
                title: String::new(),
                artist: String::new(),
                album: String::new(),
                artwork_path: None,
                duration_ms: 0,
                position_ms: 0,
                sent_at_unix_ms: now,
                playback: "stopped",
            },
        }
    }

    fn track_changed(previous: Option<&WireState>, current: &WireState) -> bool {
        previous.is_some_and(|value| value.track_id != current.track_id)
    }

    fn playback_changed(previous: Option<&WireState>, current: &WireState) -> bool {
        previous.is_some_and(|value| value.playback != current.playback)
    }

    fn read_command<R: Read>(reader: &mut R) -> io::Result<Option<(String, Command)>> {
        let Some(value) = read_frame(reader)? else {
            return Ok(None);
        };
        let envelope: IncomingEnvelope = serde_json::from_slice(&value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if envelope.version != VERSION || envelope.message_type != "command" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid envelope"));
        }
        let command = match envelope.payload.name.as_str() {
            "previous" => Command::Previous,
            "toggle_playback" => Command::TogglePlayback,
            "next" => Command::Next,
            "show_noctune" => Command::ShowNoctune,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown command")),
        };
        Ok(Some((envelope.payload.id, command)))
    }

    fn read_frame<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
        let mut prefix = [0_u8; 4];
        match reader.read_exact(&mut prefix) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error),
        }
        let length = u32::from_le_bytes(prefix) as usize;
        if length == 0 || length > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid frame size"));
        }
        let mut payload = vec![0; length];
        reader.read_exact(&mut payload)?;
        Ok(Some(payload))
    }

    fn write_state_message<W: Write>(
        writer: &mut W,
        message_type: &str,
        state: WireState,
    ) -> io::Result<()> {
        let payload = serde_json::to_value(state)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_envelope(writer, message_type, payload)
    }

    fn write_envelope<W: Write>(
        writer: &mut W,
        message_type: &str,
        payload: Value,
    ) -> io::Result<()> {
        let message = json!({ "version": VERSION, "type": message_type, "payload": payload });
        let bytes = serde_json::to_vec(&message)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if bytes.is_empty() || bytes.len() > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid frame size"));
        }
        writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&bytes)?;
        writer.flush()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Cursor;

        #[test]
        fn frame_uses_little_endian_length_and_round_trips_json() {
            let mut bytes = Vec::new();
            write_envelope(&mut bytes, "position_anchor", json!({ "positionMs": 42 }))
                .expect("write frame");

            let declared = u32::from_le_bytes(bytes[..4].try_into().expect("prefix")) as usize;
            assert_eq!(declared, bytes.len() - 4);

            let payload = read_frame(&mut Cursor::new(bytes))
                .expect("read frame")
                .expect("payload");
            let envelope: Value = serde_json::from_slice(&payload).expect("valid JSON");
            assert_eq!(envelope["version"], VERSION);
            assert_eq!(envelope["type"], "position_anchor");
        }

        #[test]
        fn frame_rejects_messages_over_limit() {
            let prefix = ((MAX_MESSAGE_SIZE + 1) as u32).to_le_bytes();
            let error = read_frame(&mut Cursor::new(prefix)).expect_err("oversized frame");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }

        #[test]
        fn pipe_name_is_stable_and_scoped_by_identity() {
            assert_eq!(
                pipe_name_for("DOMAIN", "user"),
                pipe_name_for("DOMAIN", "user")
            );
            assert_ne!(
                pipe_name_for("DOMAIN", "user"),
                pipe_name_for("DOMAIN", "other")
            );
            assert!(pipe_name_for("DOMAIN", "user").starts_with("glassline-noctune-v1-"));
        }
    }
}

#[cfg(target_os = "windows")]
pub use platform::GlasslineBridge;

#[cfg(not(target_os = "windows"))]
pub struct GlasslineBridge;

#[cfg(not(target_os = "windows"))]
impl GlasslineBridge {
    pub fn start() -> Self { Self }
    pub fn publish(&self, _snapshot: Option<PlaybackSnapshot>) {}
    pub fn drain_commands(&self) -> Vec<(String, Command)> { Vec::new() }
    pub fn complete_command(&self, _id: String, _success: bool) {}
}
