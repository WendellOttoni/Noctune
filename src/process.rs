//! Own auxiliary processes and terminate their process group/tree on cancellation.
use std::{
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub fn configure(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

pub fn terminate(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        // Only our explicitly created child process group; never the parent's group.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub struct ProbingChild {
    child: Arc<Mutex<Child>>,
    done: Arc<AtomicBool>,
}
impl ProbingChild {
    pub fn new(child: Child) -> Self {
        let child = Arc::new(Mutex::new(child));
        let done = Arc::new(AtomicBool::new(false));
        let watch_child = child.clone();
        let watch_done = done.clone();
        let cancellation = crate::worker::active_cancellation();
        std::thread::spawn(move || {
            let started = Instant::now();
            while !watch_done.load(Ordering::Acquire) {
                if cancellation.as_ref().is_some_and(|token| token.cancelled())
                    || started.elapsed() > Duration::from_secs(30)
                {
                    terminate(&mut watch_child.lock().unwrap());
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        Self { child, done }
    }
    pub fn ready(&self) {
        self.done.store(true, Ordering::Release);
    }
}
impl Drop for ProbingChild {
    fn drop(&mut self) {
        self.ready();
        terminate(&mut self.child.lock().unwrap());
    }
}
