//! The live wallpaper on macOS: one borderless window per screen at the
//! desktop picture's window level (above the wallpaper, below the desktop
//! icons), on every Space, ignoring the mouse. Its view shows the frame
//! through two IOSurfaces ([`crate::surface`]): only the rectangles that
//! changed are written, and the window server composites the surface as
//! it is, with no GPU device or swapchain in the app (they cost far more
//! memory and threads than a wallpaper needs).
//!
//! A screen whose window is fully covered (a full-screen app, the lock
//! screen, a sleeping display) reports itself occluded, and isn't drawn
//! until it shows again. A zoomed window leaves the menu bar and the Dock
//! showing, so twice a second the host also asks the window server where the
//! other apps' windows are: a screen whose usable area they all but fill
//! counts as covered too ([`crate::cover`]).
//!
//! Everything here runs on the main thread (AppKit's rule), which is where
//! the app ticks it.

use crate::cover::{self, Rect};
use crate::surface::SurfaceView;
use crate::{CoverPolicy, FrameStats, Player};
use clairlune_platform::Monitor;
use clairlune_platform::macos::find_screen;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWindowOcclusionState,
    NSWindowStyleMask,
};
use objc2_core_graphics::{
    CGWindowLevelForKey, CGWindowLevelKey, CGWindowListCopyWindowInfo, CGWindowListOption,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSPoint, NSRect, NSString};
use std::time::{Duration, Instant};

/// One screen's window and the view that shows it. The view goes before
/// the window (field order).
struct Output {
    monitor: Monitor,
    view: SurfaceView,
    covered: bool,
    window: Retained<NSWindow>,
}

/// Live wallpapers on every screen given.
pub struct MacDesktop {
    outputs: Vec<Output>,
    pub cover_policy: CoverPolicy,
    /// Per output: other apps' windows all but fill its screen.
    filled: Vec<bool>,
    /// When that was last worked out.
    checked: Option<Instant>,
}

/// How often to ask where the other apps' windows are.
pub(crate) const WINDOW_CHECK: Duration = Duration::from_millis(500);

/// Where the other apps' windows are, asked at most twice a second however
/// many want to know.
#[derive(Default)]
pub struct AppWindows {
    rects: Vec<Rect>,
    asked: Option<Instant>,
}

impl AppWindows {
    pub(crate) fn look(&mut self) -> &[Rect] {
        // A little early rather than late: two that ask in turn share one.
        let fresh = WINDOW_CHECK * 9 / 10;
        if self.asked.is_none_or(|t| t.elapsed() >= fresh) {
            self.asked = Some(Instant::now());
            self.rects = app_windows();
        }
        &self.rects
    }
}

/// The on-screen windows of other apps at the normal level (no menus,
/// panels or the Dock), in the window server's points (top-left origin).
/// Their bounds need no permission; their titles aren't read.
fn app_windows() -> Vec<Rect> {
    let options =
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(list) = CGWindowListCopyWindowInfo(options, 0) else {
        return Vec::new();
    };
    // SAFETY: a CFArray is an NSArray (toll-free bridged), and the window
    // list holds dictionaries; each item is checked before it's read.
    let list: &NSArray<AnyObject> = unsafe { &*(&*list as *const _ as *const NSArray<AnyObject>) };
    let me = f64::from(std::process::id());
    let mut out = Vec::new();
    for i in 0..list.count() {
        let item = list.objectAtIndex(i);
        let Some(d) = dict(&item) else {
            continue;
        };
        if number(d, "kCGWindowLayer") != Some(0.0)
            || number(d, "kCGWindowOwnerPID") == Some(me)
            || number(d, "kCGWindowAlpha").unwrap_or(1.0) < 0.05
        {
            continue;
        }
        let Some(bounds) = value(d, "kCGWindowBounds") else {
            continue;
        };
        let Some(b) = dict(&bounds) else {
            continue;
        };
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            number(b, "X"),
            number(b, "Y"),
            number(b, "Width"),
            number(b, "Height"),
        ) {
            out.push(Rect { x, y, w, h });
        }
    }
    out
}

fn is_kind(obj: &AnyObject, class: &objc2::runtime::AnyClass) -> bool {
    // SAFETY: isKindOfClass: is on every NSObject.
    unsafe { msg_send![obj, isKindOfClass: class] }
}

fn dict(obj: &AnyObject) -> Option<&NSDictionary<AnyObject, AnyObject>> {
    // SAFETY: checked to be a dictionary first.
    is_kind(obj, NSDictionary::<AnyObject, AnyObject>::class()).then(|| unsafe {
        &*(obj as *const AnyObject as *const NSDictionary<AnyObject, AnyObject>)
    })
}

fn value(d: &NSDictionary<AnyObject, AnyObject>, key: &str) -> Option<Retained<AnyObject>> {
    let key = NSString::from_str(key);
    let key: &AnyObject = &key;
    d.objectForKey(key)
}

fn number(d: &NSDictionary<AnyObject, AnyObject>, key: &str) -> Option<f64> {
    let v = value(d, key)?;
    // SAFETY: checked to be a number first.
    is_kind(&v, NSNumber::class())
        .then(|| unsafe { &*(&*v as *const AnyObject as *const NSNumber) }.doubleValue())
}

/// The usable part of a screen (without the menu bar and the Dock), in the
/// window server's points: AppKit's origin is the main screen's bottom left,
/// y up; the window server's its top left, y down.
pub(crate) fn usable_area(mtm: MainThreadMarker, screen: &NSScreen) -> Option<Rect> {
    let main = NSScreen::screens(mtm).firstObject()?.frame();
    let v = screen.visibleFrame();
    Some(Rect {
        x: v.origin.x,
        y: main.size.height - (v.origin.y + v.size.height),
        w: v.size.width,
        h: v.size.height,
    })
}

/// The desktop picture's window level.
pub(crate) fn desktop_level() -> isize {
    CGWindowLevelForKey(CGWindowLevelKey::DesktopWindowLevelKey) as isize
}

/// Makes a window part of the desktop: on every Space, out of the way of the
/// mouse and of Mission Control.
pub(crate) fn join_desktop(window: &NSWindow, level: isize) {
    // Owned by the Retained handle, not released again on close.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setLevel(level);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    window.setIgnoresMouseEvents(true);
    window.setHasShadow(false);
}

/// A borderless window covering the screen, at the desktop picture's level.
fn desktop_window(mtm: MainThreadMarker, monitor: &Monitor) -> Result<Retained<NSWindow>, String> {
    let screen = find_screen(mtm, &monitor.id)
        .ok_or_else(|| format!("screen {} isn't connected", monitor.id))?;
    let frame = screen.frame();
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    join_desktop(&window, desktop_level());
    window.setOpaque(true);
    window.setFrame_display(frame, false);
    Ok(window)
}

impl MacDesktop {
    /// Puts each player on its screen (the player's scene must be the
    /// screen's size in pixels) and shows its first frame. Main thread only.
    pub fn start(scenes: Vec<(Monitor, Player)>) -> Result<MacDesktop, String> {
        let mtm = MainThreadMarker::new().ok_or("live wallpapers start on the main thread")?;
        let mut outputs = Vec::new();
        for (monitor, mut player) in scenes {
            // The screen's size, or half of it (Lighter on Retina screens:
            // shown pixel-doubled).
            let (pw, ph) = player.size();
            let fits = |p: u32, m: u32| p == m || (p * 2).abs_diff(m) <= 1;
            if !(fits(pw, monitor.width) && fits(ph, monitor.height)) {
                return Err(format!(
                    "the scene is {:?} but screen {} is {}x{}",
                    player.size(),
                    monitor.id,
                    monitor.width,
                    monitor.height
                ));
            }
            let window = desktop_window(mtm, &monitor)?;
            // Sub-cell particles, as on the GPU: drawing only what changed
            // keeps a higher frame rate cheap.
            player.set_smooth(true);
            let t = player.now_ms();
            player.advance(t);
            let size = window.frame().size;
            let view = SurfaceView::new(mtm, NSRect::new(NSPoint::new(0.0, 0.0), size), player)
                .ok_or("no IOSurface for the screen")?;
            window.setContentView(Some(view.view()));
            window.orderFrontRegardless();
            outputs.push(Output {
                monitor,
                view,
                covered: false,
                window,
            });
        }
        if outputs.is_empty() {
            return Err("no screens to play on".into());
        }
        tracing::info!(target: "clairlune", screens = outputs.len(), "live: showing IOSurfaces");
        let n = outputs.len();
        Ok(MacDesktop {
            outputs,
            cover_policy: CoverPolicy::default(),
            filled: vec![false; n],
            checked: None,
        })
    }

    /// "iosurface".
    pub fn presenter(&self) -> String {
        "iosurface".into()
    }

    /// The highest frame rate worth asking for (a journey redraws the
    /// whole screen: at most Very smooth).
    pub fn max_fps(&self) -> u32 {
        if self.outputs.iter().any(|o| o.view.player().is_journey()) {
            crate::JOURNEY_MAX_FPS
        } else {
            60
        }
    }

    /// The screens playing.
    pub fn monitors(&self) -> impl Iterator<Item = &Monitor> {
        self.outputs.iter().map(|o| &o.monitor)
    }

    /// Draws the current time on every screen that shows.
    pub fn tick(&mut self, windows: &mut AppWindows) -> Result<FrameStats, String> {
        let started = Instant::now();
        if self.cover_policy == CoverPolicy::Off {
            self.filled.fill(false);
        } else if self.checked.is_none_or(|t| t.elapsed() >= WINDOW_CHECK)
            && let Some(mtm) = MainThreadMarker::new()
        {
            self.checked = Some(Instant::now());
            let windows = windows.look();
            self.filled = self
                .outputs
                .iter()
                .map(|o| {
                    o.window
                        .screen()
                        .and_then(|s| usable_area(mtm, &s))
                        .is_some_and(|a| cover::filled(a, windows))
                })
                .collect();
        }
        let shown: Vec<bool> = self
            .outputs
            .iter()
            .zip(&self.filled)
            .map(|(o, filled)| {
                !filled
                    && o.window
                        .occlusionState()
                        .contains(NSWindowOcclusionState::Visible)
            })
            .collect();
        let any_hidden = shown.iter().any(|v| !v);
        for (o, visible) in self.outputs.iter_mut().zip(&shown) {
            let covered = match self.cover_policy {
                CoverPolicy::Off => false,
                CoverPolicy::ThatMonitor => !visible,
                CoverPolicy::AllMonitors => any_hidden,
            };
            if covered != o.covered {
                if covered {
                    tracing::info!(target: "clairlune", screen = %o.monitor.id, "live: covered, paused");
                } else {
                    tracing::info!(target: "clairlune", screen = %o.monitor.id, "live: showing again, playing");
                }
            }
            o.covered = covered;
        }
        let (mut pixels, mut count) = (0u64, 0usize);
        for o in &mut self.outputs {
            if o.covered {
                continue; // nobody sees it; resumes at the time it is then
            }
            let (p, n) = o.view.advance();
            pixels += p;
            count += n;
        }
        let busy = started.elapsed();
        Ok(FrameStats {
            pixels,
            rects: count,
            covered: self.outputs.iter().filter(|o| o.covered).count(),
            away: false,
            stalls: 0,
            busy,
            upload: busy,
            present: Duration::ZERO,
        })
    }
}

impl Drop for MacDesktop {
    fn drop(&mut self) {
        for o in &self.outputs {
            o.window.orderOut(None);
            o.window.close();
        }
    }
}
