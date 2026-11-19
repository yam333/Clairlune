//! Windows: `IDesktopWallpaper` (Windows 8+), one image per monitor.
//!
//! Monitor ids are device paths (`\\?\DISPLAY#...`), stable across reboots.
//! Rectangles are physical pixels once the process is Per-Monitor DPI Aware v2.

use crate::{Capabilities, Monitor, PlatformError, Snapshot, WallpaperHost, sort_monitors};
use std::path::{Path, PathBuf};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    DESKTOP_WALLPAPER_POSITION, DWPOS_FILL, DesktopWallpaper, FOLDERID_LocalAppData,
    IDesktopWallpaper, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
};
use windows::core::{HSTRING, PWSTR};

pub(crate) fn init_dpi_awareness() {
    // Fails harmlessly when already set (by a manifest or an earlier call).
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// `%LOCALAPPDATA%`, asked of the shell rather than the environment.
pub(crate) fn local_app_data() -> Option<PathBuf> {
    let p = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }.ok()?;
    let s = take_pwstr(p);
    (!s.is_empty()).then(|| PathBuf::from(s))
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
            let current = self.current(&m)?;
            monitors.push((m.id, current));
        }
        let position = unsafe { self.dw.GetPosition() }.ok().map(|p| p.0);
        Ok(Snapshot { monitors, position })
    }

    fn restore(&self, snapshot: &Snapshot) -> Result<(), PlatformError> {
        let connected = self.monitors()?;
        if let Some(p) = snapshot.position {
            unsafe { self.dw.SetPosition(DESKTOP_WALLPAPER_POSITION(p)) }.map_err(os)?;
        }
        for (id, path) in &snapshot.monitors {
            if !connected.iter().any(|m| &m.id == id) {
                continue;
            }
            // An empty path means "no image" (the background colour).
            let path = path.as_deref().map(|p| p.as_os_str()).unwrap_or_default();
            unsafe {
                self.dw
                    .SetWallpaper(&HSTRING::from(id.as_str()), &HSTRING::from(path))
            }
            .map_err(os)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
