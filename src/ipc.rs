//! Local IPC server and CLI client for headless control (#135).

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const DEFAULT_IPC_PORT: u16 = 4848;

#[derive(Debug)]
pub enum IpcCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Volume(String),
    Status,
    StatusJson,
}

pub struct IpcRequest {
    pub command: IpcCommand,
    pub reply_tx: Sender<String>,
}

fn ipc_port_file() -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|dirs| dirs.config_dir().join("noctune.port"))
}

pub struct IpcServer {
    pub rx: Receiver<IpcRequest>,
}

impl IpcServer {
    pub fn start() -> Option<Self> {
        let (tx, rx) = mpsc::channel();

        // Try binding default port or any free port on localhost
        let listener = TcpListener::bind(format!("127.0.0.1:{DEFAULT_IPC_PORT}"))
            .or_else(|_| TcpListener::bind("127.0.0.1:0"))
            .ok()?;

        let port = listener.local_addr().ok()?.port();
        if let Some(port_file) = ipc_port_file() {
            if let Some(parent) = port_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&port_file, port.to_string());
        }

        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                thread::spawn(move || {
                    let _ = handle_client(stream, tx);
                });
            }
        });

        Some(Self { rx })
    }
}

fn handle_client(mut stream: TcpStream, tx: Sender<IpcRequest>) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let trimmed = line.trim();
    let cmd = match trimmed {
        "play" => IpcCommand::Play,
        "pause" => IpcCommand::Pause,
        "toggle" | "play-pause" => IpcCommand::Toggle,
        "next" => IpcCommand::Next,
        "prev" | "previous" => IpcCommand::Prev,
        "stop" => IpcCommand::Stop,
        "status" => IpcCommand::Status,
        "status-json" | "status --json" => IpcCommand::StatusJson,
        s if s.starts_with("volume") => {
            let arg = s.strip_prefix("volume").unwrap_or("").trim().to_string();
            IpcCommand::Volume(arg)
        }
        _ => {
            let _ = stream.write_all(b"ERROR: Unknown command\n");
            return Ok(());
        }
    };

    let (reply_tx, reply_rx) = mpsc::channel();
    let _ = tx.send(IpcRequest {
        command: cmd,
        reply_tx,
    });

    if let Ok(reply) = reply_rx.recv_timeout(Duration::from_secs(2)) {
        let _ = stream.write_all(reply.as_bytes());
        if !reply.ends_with('\n') {
            let _ = stream.write_all(b"\n");
        }
    }

    Ok(())
}

pub struct IpcClient;

impl IpcClient {
    pub fn send_command(cmd: &str) -> Result<String, String> {
        let port = if let Some(port_file) = ipc_port_file() {
            if let Ok(content) = std::fs::read_to_string(port_file) {
                content.trim().parse::<u16>().unwrap_or(DEFAULT_IPC_PORT)
            } else {
                DEFAULT_IPC_PORT
            }
        } else {
            DEFAULT_IPC_PORT
        };

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
            .map_err(|e| format!("Could not connect to Noctune instance on port {port}: {e}"))?;

        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

        stream
            .write_all(format!("{cmd}\n").as_bytes())
            .map_err(|e| format!("Failed to send command: {e}"))?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| format!("Failed to read response: {e}"))?;

        Ok(response.trim().to_string())
    }
}
