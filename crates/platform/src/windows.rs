//! Windows: `IDesktopWallpaper` (Windows 8+), one image per monitor.
//!
//! Monitor ids are device paths (`\\?\DISPLAY#...`), stable across reboots.
//! Rectangles are physical pixels once the process is Per-Monitor DPI Aware v2.

use crate::{Capabilities, Monitor, PlatformError, Snapshot, WallpaperHost, sort_monitors};
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
    RegSetKeyValueW,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    DESKTOP_WALLPAPER_POSITION, DWPOS_FILL, DesktopWallpaper, FOLDERID_LocalAppData,
    FOLDERID_Pictures, FOLDERID_RoamingAppData, IDesktopWallpaper, KF_FLAG_DEFAULT,
    SHGetKnownFolderPath,
};
use windows::core::{GUID, w};
use windows::core::{HSTRING, PWSTR};

/// Writes to the console of the process that started us, if any (a GUI-built
/// exe has none of its own), so `--version` shows in a terminal.
pub(crate) fn attach_parent_console() {
    use ::windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // Fails harmlessly when there's no parent console (started from Explorer).
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

pub(crate) fn init_dpi_awareness() {
    // Fails harmlessly when already set (by a manifest or an earlier call).
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// A known folder, asked of the shell rather than the environment.
fn known_folder(id: &GUID) -> Option<PathBuf> {
    let p = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok()?;
    let s = take_pwstr(p);
    (!s.is_empty()).then(|| PathBuf::from(s))
}

/// `%LOCALAPPDATA%`.
pub(crate) fn local_app_data() -> Option<PathBuf> {
    known_folder(&FOLDERID_LocalAppData)
}

/// `%APPDATA%` (roams with the profile: small settings only).
pub(crate) fn roaming_app_data() -> Option<PathBuf> {
    known_folder(&FOLDERID_RoamingAppData)
}

pub(crate) fn pictures() -> Option<PathBuf> {
    known_folder(&FOLDERID_Pictures)
}

/// `AppsUseLightTheme` = 0 means dark. Missing (older Windows) means unknown.
/// The per-user list of programs started at login.
const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");

fn run_value_name() -> HSTRING {
    HSTRING::from(clairlune_scene::APP_NAME)
}

/// The command registered to start at login, if any.
pub(crate) fn start_at_login_command() -> Option<String> {
    let name = run_value_name();
    let mut size = 0u32;
    let first = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            &name,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if first.is_err() || size < 2 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            &name,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Registers (or with `None` removes) the command started at login.
pub(crate) fn set_start_at_login_command(command: Option<&str>) -> Result<(), PlatformError> {
    let name = run_value_name();
    let status = match command {
        Some(cmd) => {
            let wide: Vec<u16> = cmd.encode_utf16().chain([0]).collect();
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN_KEY,
                    &name,
                    REG_SZ.0,
                    Some(wide.as_ptr() as *const core::ffi::c_void),
                    (wide.len() * 2) as u32,
                )
            }
        }
        None => match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, &name) } {
            e if e == ERROR_FILE_NOT_FOUND => return Ok(()),
            e => e,
        },
    };
    status
        .ok()
        .map_err(|e| PlatformError::Os(format!("start at login: {}", e.message())))
}

pub(crate) fn apps_use_dark_theme() -> Option<bool> {
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value == 0)
}

pub(crate) struct WindowsHost {
    dw: IDesktopWallpaper,
}

fn os(e: windows::core::Error) -> PlatformError {
    PlatformError::Os(format!("{} ({:#010x})", e.message(), e.code().0))
}

/// Copies a COM-allocated string and frees it.
fn take_pwstr(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

impl WindowsHost {
    pub(crate) fn new() -> Result<Self, PlatformError> {
        // S_FALSE (already initialised on this thread) and RPC_E_CHANGED_MODE
        // (initialised as multithreaded by someone else) both leave COM usable.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let dw: IDesktopWallpaper =
            unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) }.map_err(os)?;
        Ok(WindowsHost { dw })
    }
}

impl WallpaperHost for WindowsHost {
    fn name(&self) -> &'static str {
        "windows-desktop-wallpaper"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            per_monitor: true,
            live: false,
        }
    }

    fn monitors(&self) -> Result<Vec<Monitor>, PlatformError> {
        let count = unsafe { self.dw.GetMonitorDevicePathCount() }.map_err(os)?;
        let mut out = Vec::new();
        for i in 0..count {
            let id = take_pwstr(unsafe { self.dw.GetMonitorDevicePathAt(i) }.map_err(os)?);
            // Detached monitors stay in the list but have no rectangle.
            let Ok(r) = (unsafe { self.dw.GetMonitorRECT(&HSTRING::from(id.as_str())) }) else {
                continue;
            };
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            if id.is_empty() || w <= 0 || h <= 0 {
                continue;
            }
            out.push(Monitor {
                id,
                x: r.left,
                y: r.top,
                width: w as u32,
                height: h as u32,
                // The primary monitor's top-left is the virtual screen origin.
                primary: r.left == 0 && r.top == 0,
            });
        }
        sort_monitors(&mut out);
        Ok(out)
    }

    fn current(&self, monitor: &Monitor) -> Result<Option<PathBuf>, PlatformError> {
        let p = unsafe { self.dw.GetWallpaper(&HSTRING::from(monitor.id.as_str())) }.map_err(os)?;
        let s = take_pwstr(p);
        Ok((!s.is_empty()).then(|| PathBuf::from(s)))
    }

    fn set_static(&self, monitor: &Monitor, path: &Path) -> Result<(), PlatformError> {
        if !path.is_absolute() {
            return Err(PlatformError::Os(format!(
                "wallpaper path must be absolute: {}",
                path.display()
            )));
        }
        // Our images match each monitor's size, so Fill shows them 1:1; it also
        // covers the monitor if its mode changed since rendering.
        unsafe { self.dw.SetPosition(DWPOS_FILL) }.map_err(os)?;
        unsafe {
            self.dw.SetWallpaper(
                &HSTRING::from(monitor.id.as_str()),
                &HSTRING::from(path.as_os_str()),
            )
        }
        .map_err(os)
    }

    fn snapshot(&self) -> Result<Snapshot, PlatformError> {
        let mut monitors = Vec::new();
        for m in self.monitors()? {
            // One that won't say is left out (not taken for "no image"):
            // what was saved for it stays.
            if let Ok(current) = self.current(&m) {
                monitors.push((m.id, current));
            }
        }
        let position = unsafe { self.dw.GetPosition() }.ok().map(|p| p.0);
        Ok(Snapshot {
            monitors,
            position,
            settings: Vec::new(),
        })
    }

    fn restore(&self, snapshot: &Snapshot) -> Result<(), PlatformError> {
        let connected = self.monitors()?;
        if let Some(p) = snapshot.position {
            unsafe { self.dw.SetPosition(DESKTOP_WALLPAPER_POSITION(p)) }.map_err(os)?;
        }
        // Every monitor gets its own back, even when one fails.
        let mut first_error = None;
        for (id, path) in &snapshot.monitors {
            if !connected.iter().any(|m| &m.id == id) {
                continue;
            }
            // An image that is gone can't come back: that monitor keeps
            // what it shows.
            if path.as_deref().is_some_and(|p| !p.exists()) {
                continue;
            }
            // An empty path means "no image" (the background colour).
            let path = path.as_deref().map(|p| p.as_os_str()).unwrap_or_default();
            let set = unsafe {
                self.dw
                    .SetWallpaper(&HSTRING::from(id.as_str()), &HSTRING::from(path))
            };
            if let Err(e) = set {
                first_error.get_or_insert(os(e));
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folders_exist_and_theme_reads() {
        for dir in [roaming_app_data(), pictures()] {
            let dir = dir.expect("known folder");
            assert!(dir.is_absolute(), "{}", dir.display());
        }
        // Present on Windows 10 1809+; either answer is fine.
        let _ = apps_use_dark_theme();
    }

    #[test]
    fn reads_the_login_items_without_changing_them() {
        // Read-only: whatever is registered (often nothing) must read cleanly.
        let _ = start_at_login_command();
    }

    /// Writes the user's login items, so it only runs when asked:
    /// `cargo test -p clairlune-platform -- --ignored`. Restores what was there.
    #[test]
    #[ignore]
    fn login_item_round_trip() {
        let before = start_at_login_command();
        let fake = r#""C:\nowhere\clairlune-test.exe""#;
        set_start_at_login_command(Some(fake)).unwrap();
        assert_eq!(start_at_login_command().as_deref(), Some(fake));
        set_start_at_login_command(before.as_deref()).unwrap();
        assert_eq!(start_at_login_command(), before);
        // Removing a value that isn't there is fine.
        if before.is_none() {
            set_start_at_login_command(None).unwrap();
        }
    }

    #[test]
    fn local_app_data_exists() {
        let dir = local_app_data().expect("Local AppData");
        assert!(dir.is_absolute() && dir.is_dir(), "{}", dir.display());
    }

    /// Read-only: lists monitors and reads their wallpapers. CI machines may
    /// have no desktop; then the host or the list is simply empty.
    #[test]
    fn monitors_have_ids_and_sizes() {
        init_dpi_awareness();
        let Ok(host) = WindowsHost::new() else {
            return;
        };
        let Ok(list) = host.monitors() else {
            return;
        };
        let mut ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), list.len(), "monitor ids are unique");
        assert!(list.iter().filter(|m| m.primary).count() <= 1);
        if let Some(first) = list.first() {
            assert!(first.width > 0 && first.height > 0);
            assert!(host.current(first).is_ok());
        }
    }
}
