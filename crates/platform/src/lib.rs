//! Platform layer: finding the monitors and setting the desktop wallpaper.
//!
//! The OS wallpaper APIs only set static images. A
//! [`WallpaperHost`] wraps one OS's API: list the monitors with stable ids and
//! physical sizes, read and set each monitor's wallpaper. [`detect`] picks the
//! host for the running system. [`StillStore`] writes each still under a new
//! file name, because some systems don't reload an unchanged path.
//!
//! Windows (`IDesktopWallpaper`, per monitor) and macOS (`NSWorkspace`, per
//! screen) are implemented. Elsewhere [`detect`] says the wallpaper isn't
//! set here: the scenes still render (`clairlune-cli render`, `gif`).

mod login;
/// macOS: the wallpaper per screen, and Space changes.
#[cfg(target_os = "macos")]
pub mod macos;
mod power;
mod store;
/// What this process uses (CPU time, memory), for the log.
pub mod usage;
#[cfg(windows)]
mod windows;
/// Live wallpaper windows in the desktop layer (Windows).
#[cfg(windows)]
pub mod windows_live;

pub use login::BACKGROUND_ARG;
pub use power::on_battery;
pub use store::{Snapshot, StillStore};

use std::fmt;
use std::path::{Path, PathBuf};

/// One monitor: its size in physical pixels, and where it sits in the
/// system's display arrangement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monitor {
    /// Stable id that survives reboots and re-plugging: the device path on
    /// Windows. Per-monitor seeds and saved scenes key on it.
    pub id: String,
    /// Its top-left corner in the system's display arrangement (the origin
    /// can be negative), in the arrangement's units: pixels on Windows,
    /// points on macOS.
    pub x: i32,
    pub y: i32,
    /// Its size in physical pixels.
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    /// Physical pixels per arrangement unit: 2 on a Retina screen, 1
    /// elsewhere (and always on Windows, which arranges in pixels).
    pub scale: u32,
}

impl Monitor {
    /// Its size in the arrangement's units (points on macOS).
    pub fn arranged_size(&self) -> (u32, u32) {
        let s = self.scale.max(1);
        (self.width.div_ceil(s), self.height.div_ceil(s))
    }
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
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacHost::new()?))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(PlatformError::Unsupported(format!(
            "Clairlune sets the wallpaper on Windows and macOS, not on {}; `render` and `gif` make the images",
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

/// The command line that starts `exe` at login, quietly.
#[cfg_attr(not(windows), allow(dead_code))]
fn login_command(exe: &Path) -> String {
    format!("\"{}\" {BACKGROUND_ARG}", exe.display())
}

/// The command 0.12 and older registered: no background argument.
#[cfg_attr(not(windows), allow(dead_code))]
fn old_login_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Whether `exe` is registered to start at login; `None` where this build
/// can't manage login items.
pub fn start_at_login(exe: &Path) -> Option<bool> {
    #[cfg(windows)]
    {
        let (want, old) = (login_command(exe), old_login_command(exe));
        Some(
            windows::start_at_login_command()
                .is_some_and(|c| c.eq_ignore_ascii_case(&want) || c.eq_ignore_ascii_case(&old)),
        )
    }
    #[cfg(target_os = "macos")]
    {
        Some(login::registered(exe))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = exe;
        None
    }
}

/// Whether the login entry for `exe` is one from 0.12 or older (Windows),
/// which starts it without [`BACKGROUND_ARG`].
pub fn login_entry_outdated(exe: &Path) -> bool {
    #[cfg(windows)]
    {
        windows::start_at_login_command()
            .is_some_and(|c| c.eq_ignore_ascii_case(&old_login_command(exe)))
    }
    #[cfg(not(windows))]
    {
        let _ = exe;
        false
    }
}

/// Start at login is registered but switched off where only the user can
/// switch it on: macOS's System Settings > General > Login Items, or
/// Windows' Settings > Apps > Startup. `false` elsewhere.
pub fn start_at_login_needs_approval(exe: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        login::needs_approval(exe)
    }
    #[cfg(windows)]
    {
        let _ = exe;
        windows::startup_switched_off()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = exe;
        false
    }
}

/// Opens where the user switches start at login on: System Settings >
/// General > Login Items (macOS), Settings > Apps > Startup (Windows).
/// Nothing elsewhere.
pub fn open_login_items_settings() {
    #[cfg(target_os = "macos")]
    login::open_login_items();
    #[cfg(windows)]
    let _ = windows::open_url("ms-settings:startupapps");
}

/// Registers or removes `exe` as a login item (per user, no admin rights),
/// started with [`BACKGROUND_ARG`].
pub fn set_start_at_login(exe: &Path, enable: bool) -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        let cmd = login_command(exe);
        windows::set_start_at_login_command(enable.then_some(cmd.as_str()))
    }
    #[cfg(target_os = "macos")]
    {
        login::set(exe, enable)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (exe, enable);
        Err(PlatformError::Unsupported(
            "starting at login isn't supported on this system".into(),
        ))
    }
}

/// Whether the taskbar (where the tray icon sits) is light, when that is
/// known: Windows only (macOS tints a template icon itself).
pub fn taskbar_is_light() -> Option<bool> {
    #[cfg(windows)]
    {
        windows::taskbar_is_light()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Has the system look at apps' icons again: Windows remembers an exe's by its
/// path, so after an update the taskbar kept the older version's. Elsewhere
/// there's nothing to do.
pub fn refresh_icons() {
    #[cfg(windows)]
    windows::refresh_icons();
}

/// Draws pop-up menus (the tray's) dark or light, or as apps are (`None`).
/// Windows only; macOS menus follow the system by themselves.
pub fn set_menu_theme(dark: Option<bool>) {
    #[cfg(windows)]
    windows::set_menu_theme(dark);
    #[cfg(not(windows))]
    let _ = dark;
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

/// Runs `f` inside an Objective-C autorelease pool on macOS (winit's timer
/// callbacks run outside `NSApplication`'s event pool), or directly elsewhere.
pub fn with_autoreleasepool<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(target_os = "macos")]
    {
        objc2::rc::autoreleasepool(|_| f())
    }
    #[cfg(not(target_os = "macos"))]
    {
        f()
    }
}

/// Whether the user asked the operating system to reduce UI motion/animations.
pub fn reduce_motion() -> bool {
    #[cfg(windows)]
    {
        use ::windows::Win32::UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        };
        let mut animate: i32 = 1;
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&raw mut animate).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        ok.is_ok() && animate == 0
    }
    #[cfg(target_os = "macos")]
    {
        let ws = objc2_app_kit::NSWorkspace::sharedWorkspace();
        unsafe { objc2::msg_send![&ws, accessibilityDisplayShouldReduceMotion] }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
}

/// Opens a web page in the default browser (`&` and `%` in it are safe).
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::open_url(url)
    }
    #[cfg(not(windows))]
    {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(opener)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Lets a GUI-built exe print to the terminal it was started from (Windows;
/// elsewhere the process already shares it).
#[cfg(windows)]
pub fn attach_parent_console() {
    windows::attach_parent_console();
}

/// Sorts monitors primary first, then left to right, top to bottom, so the
/// order is the same on every run.
#[cfg(any(windows, target_os = "macos", test))]
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
            scale: 1,
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
