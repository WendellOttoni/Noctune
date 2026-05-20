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

#[cfg(target_os = "windows")]
mod win {
    use anyhow::{anyhow, Result};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, HWND_MESSAGE,
        WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW,
    };

    /// Hidden message-only window — souvlaki uses its HWND as the SMTC handle.
    pub struct HiddenWindow {
        pub hwnd: HWND,
    }

    impl HiddenWindow {
        pub fn new() -> Result<Self> {
            unsafe {
                let class_name: Vec<u16> = "NoctuneMediaSession\0".encode_utf16().collect();
                let hinstance = GetModuleHandleW(None)
                    .map_err(|e| anyhow!("GetModuleHandleW: {e:?}"))?;

                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: hinstance.into(),
                    lpszClassName: PCWSTR(class_name.as_ptr()),
                    ..Default::default()
                };
                // Re-registering the same class returns 0 but sets ERROR_CLASS_ALREADY_EXISTS.
                // That's fine — we just need the class to exist by the time CreateWindowExW runs.
                let _ = RegisterClassExW(&wc);

                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    PCWSTR(class_name.as_ptr()),
                    PCWSTR(class_name.as_ptr()),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    Some(hinstance.into()),
                    None,
                )
                .map_err(|e| anyhow!("CreateWindowExW: {e:?}"))?;

                Ok(Self { hwnd })
            }
        }
    }

    impl Drop for HiddenWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

pub struct MediaSession {
    controls: MediaControls,
    #[cfg(target_os = "windows")]
    _hwnd: win::HiddenWindow,
}

impl MediaSession {
    /// Build the session and return a channel that surfaces user actions issued from
    /// the OS card (play, pause, next, previous, …).
    pub fn new(app_name: &str) -> Result<(Self, Receiver<MediaControlEvent>)> {
        #[cfg(target_os = "windows")]
        let hidden = win::HiddenWindow::new()?;

        #[cfg(target_os = "windows")]
        let config = PlatformConfig {
            dbus_name: "noctune",
            display_name: app_name,
            hwnd: Some(hidden.hwnd.0 as *mut std::ffi::c_void),
        };
        #[cfg(not(target_os = "windows"))]
        let config = PlatformConfig {
            dbus_name: "noctune",
            display_name: app_name,
            hwnd: None,
        };

        let mut controls = MediaControls::new(config)
            .map_err(|e| anyhow::anyhow!("media-session init: {e:?}"))?;
        let (tx, rx) = std::sync::mpsc::channel();
        controls
            .attach(move |event| { let _ = tx.send(event); })
            .map_err(|e| anyhow::anyhow!("media-session attach: {e:?}"))?;

        Ok((
            Self {
                controls,
                #[cfg(target_os = "windows")]
                _hwnd: hidden,
            },
            rx,
        ))
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
