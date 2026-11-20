//! Platform layer: finding the monitors and setting the desktop wallpaper.
//!
//! The OS wallpaper APIs only set static images (PLAN.md section 2). A
//! [`WallpaperHost`] wraps one OS's API: list the monitors with stable ids and
//! physical sizes, read and set each monitor's wallpaper. [`detect`] picks the
//! host for the running system. [`StillStore`] writes each still under a new
//! file name, because some systems don't reload an unchanged path.
//!
//! Windows (`IDesktopWallpaper`, per monitor) and Linux (GNOME, Cinnamon,
//! MATE, KDE Plasma, XFCE, Sway; one image for all monitors) are implemented;

#[cfg(target_os = "linux")]
mod linux;
/// macOS: the wallpaper per screen, and Space changes.
#[cfg(target_os = "macos")]
pub mod macos;
mod store;
#[cfg(windows)]
mod windows;
/// Live wallpaper windows in the desktop layer (Windows).
#[cfg(windows)]
pub mod windows_live;

pub use store::{Snapshot, StillStore};

use std::fmt;
use std::path::{Path, PathBuf};

/// One monitor, in physical pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monitor {
    /// Stable id that survives reboots and re-plugging: the device path on
    /// Windows. Per-monitor seeds and saved scenes key on it (PLAN.md 2.1).
    pub id: String,
    /// Position on the virtual screen (the origin can be negative).
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// What a host can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// A different image per monitor.
    pub per_monitor: bool,
    /// An animated host behind the desktop icons (later milestone).
    pub live: bool,
}

#[derive(Debug)]
pub enum PlatformError {
    /// No host for this system yet, with a reason for the user.
    Unsupported(String),
    /// The OS call failed.
    Os(String),
    Io(std::io::Error),
    /// The monitor id is not (or no longer) connected.
    NoSuchMonitor(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Unsupported(why) => write!(f, "not supported here: {why}"),
            PlatformError::Os(e) => write!(f, "system call failed: {e}"),
            PlatformError::Io(e) => write!(f, "{e}"),
            PlatformError::NoSuchMonitor(id) => write!(f, "no connected monitor `{id}`"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<std::io::Error> for PlatformError {
    fn from(e: std::io::Error) -> Self {
        PlatformError::Io(e)
    }
}

/// One system's wallpaper API.
pub trait WallpaperHost {
    /// Short name for logs and the settings screen, e.g. `windows-desktop-wallpaper`.
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    /// Connected monitors, primary first.
    fn monitors(&self) -> Result<Vec<Monitor>, PlatformError>;
    /// The image currently set on a monitor, if any (a solid colour has none).
    fn current(&self, monitor: &Monitor) -> Result<Option<PathBuf>, PlatformError>;
    /// Sets a still image on one monitor. The path must be absolute, and the
    /// image should already be the monitor's size.
    fn set_static(&self, monitor: &Monitor, path: &Path) -> Result<(), PlatformError>;
    /// Everything needed to put the current wallpaper back later.
    fn snapshot(&self) -> Result<Snapshot, PlatformError>;
    /// Puts a snapshot back. Monitors that are gone are skipped.
    fn restore(&self, snapshot: &Snapshot) -> Result<(), PlatformError>;
}

/// The host for the running system.
pub fn detect() -> Result<Box<dyn WallpaperHost>, PlatformError> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::WindowsHost::new()?))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxHost::new()?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacHost::new()?))
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Err(PlatformError::Unsupported(format!(
            "setting the wallpaper on {} arrives in a later milestone",
            std::env::consts::OS
        )))
    }
}

/// The per-user local data directory: Local AppData on Windows,
/// `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` (or
/// `~/.local/share`) elsewhere.
pub fn local_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows::local_app_data()
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home_dir().map(|h| h.join(".local").join("share")))
    }
}

/// The per-user configuration directory: Roaming AppData on Windows,
/// `~/Library/Application Support` on macOS, `$XDG_CONFIG_HOME` (or
/// `~/.config`) elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows::roaming_app_data()
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home_dir().map(|h| h.join(".config")))
    }
}

/// The user's Pictures folder (exports go to `Pictures/Clairlune`).
pub fn pictures_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows::pictures()
    }
    #[cfg(unix)]
    {
        home_dir().map(|h| h.join("Pictures"))
    }
}

/// Clairlune's data directory (stills, archive, logs): `<local data>/clairlune`.
pub fn app_data_dir() -> Option<PathBuf> {
    local_data_dir().map(|d| d.join(clairlune_scene::ident::APP_ID))
}

/// Clairlune's configuration directory: `<config>/clairlune`.
pub fn app_config_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join(clairlune_scene::ident::APP_ID))
}

/// The command line that starts `exe` at login.
fn login_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Whether `exe` is registered to start at login; `None` where this build
/// can't manage login items yet.
pub fn start_at_login(exe: &Path) -> Option<bool> {
    #[cfg(windows)]
    {
        let want = login_command(exe);
        Some(windows::start_at_login_command().is_some_and(|c| c.eq_ignore_ascii_case(&want)))
    }
    #[cfg(not(windows))]
    {
        let _ = login_command(exe);
        None
    }
}

/// Registers or removes `exe` as a login item (per user, no admin rights).
pub fn set_start_at_login(exe: &Path, enable: bool) -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        let cmd = login_command(exe);
        windows::set_start_at_login_command(enable.then_some(cmd.as_str()))
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, enable);
        Err(PlatformError::Unsupported(
            "starting at login arrives with the installers".into(),
        ))
    }
}

/// Whether the system asks apps for a dark appearance, when that is known.
pub fn system_prefers_dark() -> Option<bool> {
    #[cfg(windows)]
    {
        windows::apps_use_dark_theme()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(unix)]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Makes the process aware of each monitor's DPI so sizes come back in physical
/// pixels. Call once, before any window or monitor query. A no-op where it
/// does not apply.
pub fn init_dpi_awareness() {
    #[cfg(windows)]
    windows::init_dpi_awareness();
}

/// Sorts monitors primary first, then left to right, top to bottom, so the
/// order is the same on every run.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows host for now
pub(crate) fn sort_monitors(monitors: &mut [Monitor]) {
    monitors.sort_by_key(|m| (!m.primary, m.x, m.y));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, x: i32, primary: bool) -> Monitor {
        Monitor {
            id: id.into(),
            x,
            y: 0,
            width: 1920,
            height: 1080,
            primary,
        }
    }

    #[test]
    fn primary_first_then_left_to_right() {
        let mut v = vec![m("c", 1920, false), m("b", -1920, false), m("a", 0, true)];
        sort_monitors(&mut v);
        let ids: Vec<&str> = v.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
    }
}
