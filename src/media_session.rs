//! OS media-session integration (#54) — shows Noctune in Windows SMTC, Linux MPRIS,
//! and macOS MediaRemote. Built on top of `souvlaki` so the same code drives all
//! three platforms.
//!
//! The app forwards track changes / play-pause state via `MediaSession::update_*`
//! and consumes user actions (play/pause/next/prev from the OS card) via the
//! `mpsc::Receiver` returned by `spawn`.

use anyhow::Result;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use std::{sync::mpsc::Receiver, time::Duration};

#[cfg(target_os = "windows")]
mod win {
    use anyhow::{anyhow, Result};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, WNDCLASSEXW, WS_POPUP,
    };

    const AUMID: &str = "dev.noctune.Noctune";
    const DISPLAY_NAME: &str = "Noctune";

    /// SMTC on Windows refuses to attach unless the calling process has an
    /// AppUserModelID set. Console apps don't get one by default, so we declare
    /// one ourselves before souvlaki creates the media controls, and register a
    /// friendly DisplayName under HKCU so the SMTC card / Volume flyout show
    /// "Noctune" instead of "Unknown app".
    fn ensure_app_user_model_id() {
        register_aumid_display_name();
        let id: Vec<u16> = format!("{AUMID}\0").encode_utf16().collect();
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
        }
    }

    /// Writes `HKCU\Software\Classes\AppUserModelId\<AUMID>\DisplayName` so the
    /// OS can resolve our AUMID to the string "Noctune". No admin needed —
    /// HKCU is per-user. Idempotent; safe to run on every launch.
    fn register_aumid_display_name() {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, REG_SZ,
        };
        let subkey: Vec<u16> = format!("Software\\Classes\\AppUserModelId\\{AUMID}\0")
            .encode_utf16()
            .collect();
        let value_name: Vec<u16> = "DisplayName\0".encode_utf16().collect();
        let display: Vec<u16> = format!("{DISPLAY_NAME}\0").encode_utf16().collect();
        unsafe {
            let mut hkey: HKEY = std::ptr::null_mut();
            let status = RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut hkey);
            if status == 0 {
                let bytes = (display.len() * 2) as u32;
                let _ = RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    display.as_ptr() as *const u8,
                    bytes,
                );
                let _ = RegCloseKey(hkey);
            }
        }
    }

    /// Hidden message-only window — souvlaki uses its HWND as the SMTC handle.
    pub struct HiddenWindow {
        pub hwnd: HWND,
    }

    impl HiddenWindow {
        pub fn new() -> Result<Self> {
            ensure_app_user_model_id();
            unsafe {
                let class_name: Vec<u16> = "NoctuneMediaSession\0".encode_utf16().collect();
                let hinstance = GetModuleHandleW(std::ptr::null());
                if hinstance.is_null() {
                    return Err(anyhow!("GetModuleHandleW returned null"));
                }

                let mut wc: WNDCLASSEXW = std::mem::zeroed();
                wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
                wc.lpfnWndProc = Some(wnd_proc);
                wc.hInstance = hinstance as _;
                wc.lpszClassName = class_name.as_ptr();
                // Re-registering the same class returns 0 + ERROR_CLASS_ALREADY_EXISTS.
                // We don't care — we only need the class to exist before CreateWindowExW.
                let _ = RegisterClassExW(&wc);

                // SMTC's ISystemMediaTransportControlsInterop::GetForWindow rejects
                // HWND_MESSAGE windows with E_INVALIDARG — it wants a real top-level
                // window on the desktop. We create a 0×0 popup that never gets shown,
                // which satisfies the API without ever appearing in the user's taskbar
                // or alt-tab list.
                let hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    class_name.as_ptr(),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    hinstance as _,
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return Err(anyhow!("CreateWindowExW returned null"));
                }

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
            hwnd: Some(hidden.hwnd as *mut std::ffi::c_void),
        };
        #[cfg(not(target_os = "windows"))]
        let config = PlatformConfig {
            dbus_name: "noctune",
            display_name: app_name,
            hwnd: None,
        };

        let mut controls =
            MediaControls::new(config).map_err(|e| anyhow::anyhow!("media-session init: {e:?}"))?;
        let (tx, rx) = std::sync::mpsc::channel();
        controls
            .attach(move |event| {
                let _ = tx.send(event);
            })
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

    pub fn update_metadata(
        &mut self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration: Option<Duration>,
    ) {
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
            MediaPlayback::Playing {
                progress: Some(pos),
            }
        } else {
            MediaPlayback::Paused {
                progress: Some(pos),
            }
        };
        let _ = self.controls.set_playback(state);
    }

    pub fn stopped(&mut self) {
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
    }
}

/// Pump pending Win32 messages for the hidden window so souvlaki's SMTC
/// callbacks fire. Without this the OS sees us register a media session but
/// never delivers play/pause/next/prev events — and the SMTC card may not
/// surface in the volume flyout. Cheap to call from the main loop; PeekMessage
/// returns immediately when the queue is empty.
#[cfg(target_os = "windows")]
pub fn pump_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[inline]
pub fn pump_messages() {}
