//! macOS: `NSWorkspace` sets a picture per screen. It needs no permission.
//!
//! - It sets the current Space only, so the app sets the picture again when
//!   the active Space changes ([`SpaceWatcher`]).
//! - It doesn't reload a path it already shows, so stills get unique file
//!   names (`StillStore`).
//! - AppKit's screen calls belong to the main thread; calls from other
//!   threads (the app's worker) run there, synchronously.
//!
//! Monitor ids come from the display's vendor, model and serial numbers, so
//! they survive reboots and re-plugging.

use crate::{Capabilities, Monitor, PlatformError, Snapshot, WallpaperHost, sort_monitors};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSScreen, NSWorkspace};
use objc2_core_graphics::{
    CGDisplayModelNumber, CGDisplaySerialNumber, CGDisplayUnitNumber, CGDisplayVendorNumber,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Runs `f` on the main thread: here when already on it, else through the
/// main queue (the app's event loop serves it).
fn on_main<R: Send>(f: impl FnOnce(MainThreadMarker) -> R + Send) -> R {
    if let Some(mtm) = MainThreadMarker::new() {
        return f(mtm);
    }
    let mut out = None;
    dispatch2::DispatchQueue::main().exec_sync(|| {
        let mtm = MainThreadMarker::new().expect("the main queue runs on the main thread");
        out = Some(f(mtm));
    });
    out.expect("the main queue ran the work")
}

/// The display a screen shows on (`NSScreenNumber`).
fn display_id(screen: &NSScreen) -> Option<u32> {
    let desc = screen.deviceDescription();
    let value = desc.objectForKey(&NSString::from_str("NSScreenNumber"))?;
    let number = value.downcast::<NSNumber>().ok()?;
    Some(number.unsignedIntValue())
}

/// A stable id from the display's own numbers; the unit number tells apart
/// displays that report no serial.
fn stable_id(display: u32) -> String {
    let vendor = CGDisplayVendorNumber(display);
    let model = CGDisplayModelNumber(display);
    let serial = CGDisplaySerialNumber(display);
    if serial != 0 {
        format!("mac-{vendor:04x}-{model:04x}-{serial:08x}")
    } else {
        format!(
            "mac-{vendor:04x}-{model:04x}-u{}",
            CGDisplayUnitNumber(display)
        )
    }
}

/// The connected screens with their monitors, primary (menu bar) first.
fn screens(mtm: MainThreadMarker) -> Vec<(Retained<NSScreen>, Monitor)> {
    let all = NSScreen::screens(mtm);
    let main_height = all
        .firstObject()
        .map(|s| s.frame().size.height)
        .unwrap_or(0.0);
    let mut out = Vec::new();
    for (i, screen) in all.iter().enumerate() {
        let Some(display) = display_id(&screen) else {
            continue;
        };
        let f = screen.frame();
        let scale = screen.backingScaleFactor();
        // AppKit puts the origin at the primary screen's bottom left; the
        // rest of the app uses top left, in pixels.
        let top = main_height - (f.origin.y + f.size.height);
        let monitor = Monitor {
            id: stable_id(display),
            x: (f.origin.x * scale).round() as i32,
            y: (top * scale).round() as i32,
            width: (f.size.width * scale).round() as u32,
            height: (f.size.height * scale).round() as u32,
            primary: i == 0,
        };
        out.push((screen, monitor));
    }
    out
}

/// The connected screen showing the monitor with this id.
pub fn find_screen(mtm: MainThreadMarker, id: &str) -> Option<Retained<NSScreen>> {
    screens(mtm)
        .into_iter()
        .find(|(_, m)| m.id == id)
        .map(|(s, _)| s)
}

/// The host for macOS.
pub(crate) struct MacHost;

impl MacHost {
    pub(crate) fn new() -> Result<MacHost, PlatformError> {
        Ok(MacHost)
    }
}

impl WallpaperHost for MacHost {
    fn name(&self) -> &'static str {
        "macos-nsworkspace"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            per_monitor: true,
            live: false,
        }
    }

    fn monitors(&self) -> Result<Vec<Monitor>, PlatformError> {
        let mut list: Vec<Monitor> =
            on_main(|mtm| screens(mtm).into_iter().map(|(_, m)| m).collect());
        sort_monitors(&mut list);
        Ok(list)
    }

    fn current(&self, monitor: &Monitor) -> Result<Option<PathBuf>, PlatformError> {
        let id = monitor.id.clone();
        on_main(move |mtm| {
            let screen = find_screen(mtm, &id).ok_or(PlatformError::NoSuchMonitor(id))?;
            let url = NSWorkspace::sharedWorkspace().desktopImageURLForScreen(&screen);
            Ok(url
                .and_then(|u| u.path())
                .map(|p| PathBuf::from(p.to_string())))
        })
    }

    fn set_static(&self, monitor: &Monitor, path: &Path) -> Result<(), PlatformError> {
        if !path.is_absolute() {
            return Err(PlatformError::Os(format!(
                "wallpaper path must be absolute: {}",
                path.display()
            )));
        }
        let id = monitor.id.clone();
        let path = path.to_string_lossy().into_owned();
        on_main(move |mtm| {
            let screen = find_screen(mtm, &id).ok_or(PlatformError::NoSuchMonitor(id))?;
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
            // No options: the system's default fill; the image is already
            // the screen's size in pixels.
            let options = NSDictionary::new();
            unsafe {
                NSWorkspace::sharedWorkspace()
                    .setDesktopImageURL_forScreen_options_error(&url, &screen, &options)
            }
            .map_err(|e| PlatformError::Os(e.localizedDescription().to_string()))
        })
    }

    fn snapshot(&self) -> Result<Snapshot, PlatformError> {
        let mut monitors = Vec::new();
        for m in self.monitors()? {
            let current = self.current(&m)?;
            monitors.push((m.id, current));
        }
        Ok(Snapshot {
            monitors,
            position: None,
        })
    }

    fn restore(&self, snapshot: &Snapshot) -> Result<(), PlatformError> {
        let connected = self.monitors()?;
        for (id, path) in &snapshot.monitors {
            let (Some(m), Some(path)) = (connected.iter().find(|m| &m.id == id), path) else {
                continue;
            };
            self.set_static(m, path)?;
        }
        Ok(())
    }
}

/// Set when the active Space changes: the picture was set on the old Space
/// only, so it must be set again.
static SPACE_CHANGED: AtomicBool = AtomicBool::new(false);

/// Hears Space changes. Create it on the main thread and keep it.
pub struct SpaceWatcher {
    _token: Retained<objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>>,
}

impl SpaceWatcher {
    pub fn new() -> SpaceWatcher {
        use block2::RcBlock;
        use objc2_app_kit::NSWorkspaceActiveSpaceDidChangeNotification;
        use objc2_foundation::NSNotification;
        let block = RcBlock::new(|_: std::ptr::NonNull<NSNotification>| {
            SPACE_CHANGED.store(true, Ordering::Relaxed);
        });
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceActiveSpaceDidChangeNotification),
                None,
                None,
                &block,
            )
        };
        SpaceWatcher { _token: token }
    }

    /// Whether the active Space changed since the last call.
    pub fn take_changed(&self) -> bool {
        SPACE_CHANGED.swap(false, Ordering::Relaxed)
    }
}

impl Default for SpaceWatcher {
    fn default() -> Self {
        Self::new()
    }
}
