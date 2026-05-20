//! OS media-session integration (#54) — shows Noctune in Windows SMTC, Linux MPRIS,
//! and macOS MediaRemote. Built on top of `souvlaki` so the same code drives all
//! three platforms.
//!
//! The app forwards track changes / play-pause state via `MediaSession::update_*`
//! and consumes user actions (play/pause/next/prev from the OS card) via the
//! `mpsc::Receiver` returned by `spawn`.

use anyhow::Result;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};
use std::{sync::mpsc::Receiver, time::Duration};

pub struct MediaSession {
    controls: MediaControls,
}

impl MediaSession {
    /// Build the session and return a channel that surfaces user actions issued from
    /// the OS card (play, pause, next, previous, …).
    pub fn new(app_name: &str) -> Result<(Self, Receiver<MediaControlEvent>)> {
        #[cfg(target_os = "windows")]
        let hwnd = unsafe {
            // SMTC needs a window handle. We don't have one — souvlaki accepts None on
            // newer Windows builds and falls back to a hidden helper window.
            None
        };
        #[cfg(not(target_os = "windows"))]
        let hwnd: Option<()> = None;

        let config = PlatformConfig {
            dbus_name: "noctune",
            display_name: app_name,
            #[cfg(target_os = "windows")]
            hwnd,
            #[cfg(not(target_os = "windows"))]
            hwnd: hwnd.map(|_| std::ptr::null_mut()),
        };

        // souvlaki panics on Windows if it cannot get a usable HWND (we are a TUI
        // and don't own a window). Wrap the constructor in catch_unwind so the
        // failure cleanly disables the feature instead of crashing startup.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            MediaControls::new(config)
        }));
        let mut controls = match result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(anyhow::anyhow!("media-session init: {e:?}")),
            Err(_) => return Err(anyhow::anyhow!("media-session unsupported on this platform")),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        controls
            .attach(move |event| { let _ = tx.send(event); })
            .map_err(|e| anyhow::anyhow!("media-session attach: {e:?}"))?;
        Ok((Self { controls }, rx))
    }

    pub fn update_metadata(&mut self, title: &str, artist: &str, album: Option<&str>, duration: Option<Duration>) {
        let _ = self.controls.set_metadata(MediaMetadata {
            title: Some(title),
            artist: Some(artist),
            album,
            duration,
            cover_url: None,
        });
    }

    pub fn update_playback(&mut self, playing: bool, elapsed: Duration) {
        let pos = MediaPosition(elapsed);
        let state = if playing {
            MediaPlayback::Playing { progress: Some(pos) }
        } else {
            MediaPlayback::Paused { progress: Some(pos) }
        };
        let _ = self.controls.set_playback(state);
    }

    pub fn stopped(&mut self) {
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
    }
}
