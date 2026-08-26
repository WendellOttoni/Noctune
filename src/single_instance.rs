use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

pub struct SingleInstanceGuard {
    lock_path: PathBuf,
}

impl SingleInstanceGuard {
    /// Acquires a single-instance lock for the application.
    /// If another active instance of `noctune` is already running, returns `Ok(None)`.
    /// Otherwise, writes our PID and returns `Ok(Some(guard))`.
    pub fn acquire() -> Result<Option<Self>> {
        let lock_path = match crate::config::project_dirs() {
            Ok(dirs) => dirs.config_dir().join("noctune.pid"),
            Err(_) => {
                return Ok(Some(Self {
                    lock_path: PathBuf::from("noctune.pid"),
                }))
            }
        };

        if let Some(parent) = lock_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if lock_path.exists() {
            if let Ok(content) = fs::read_to_string(&lock_path) {
                if let Ok(pid_num) = content.trim().parse::<u32>() {
                    let mut sys = System::new();
                    let pid = Pid::from_u32(pid_num);
                    sys.refresh_processes_specifics(
                        ProcessesToUpdate::Some(&[pid]),
                        true,
                        ProcessRefreshKind::new(),
                    );

                    if let Some(process) = sys.process(pid) {
                        let proc_name = process.name().to_string_lossy().to_lowercase();
                        if proc_name.contains("noctune") {
                            return Ok(None);
                        }
                    }
                }
            }
        }

        let my_pid = std::process::id();
        if let Err(e) = fs::write(&lock_path, my_pid.to_string()) {
            tracing::warn!(target: "single_instance", "failed to write pid file: {e}");
        }

        Ok(Some(Self { lock_path }))
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}
