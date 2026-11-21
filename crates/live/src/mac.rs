//! The live wallpaper on macOS: one borderless window per screen at the
//! desktop picture's window level (above the wallpaper, below the desktop
//! icons), on every Space, ignoring the mouse. Its view shows the frame
//! through two IOSurfaces ([`crate::surface`]): only the rectangles that
//! changed are written, and the window server composites the surface as
//! it is, with no GPU device or swapchain in the app (they cost far more
//! memory and threads than a wallpaper needs).
//!
//! A screen whose window is fully covered (a zoomed app, the lock screen, a
//! sleeping display) reports itself occluded, and isn't drawn until it shows
//! again.
//!
//! Everything here runs on the main thread (AppKit's rule), which is where
//! the app ticks it.

use crate::surface::SurfaceView;
use crate::{CoverPolicy, FrameStats, Player};
use clairlune_platform::Monitor;
use clairlune_platform::macos::find_screen;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSWindow, NSWindowCollectionBehavior, NSWindowOcclusionState,
    NSWindowStyleMask,
};
use objc2_core_graphics::{CGWindowLevelForKey, CGWindowLevelKey};
use objc2_foundation::{NSPoint, NSRect};
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
    // Owned by the Retained handle, not released again on close.
    unsafe { window.setReleasedWhenClosed(false) };
    let level = CGWindowLevelForKey(CGWindowLevelKey::DesktopWindowLevelKey);
    window.setLevel(level as isize);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    window.setIgnoresMouseEvents(true);
    window.setHasShadow(false);
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
            if player.size() != (monitor.width, monitor.height) {
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
        Ok(MacDesktop {
            outputs,
            cover_policy: CoverPolicy::default(),
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
    pub fn tick(&mut self) -> Result<FrameStats, String> {
        let started = Instant::now();
        let shown: Vec<bool> = self
            .outputs
            .iter()
            .map(|o| {
                o.window
                    .occlusionState()
                    .contains(NSWindowOcclusionState::Visible)
            })
            .collect();
        let any_hidden = shown.iter().any(|v| !v);
        for (o, visible) in self.outputs.iter_mut().zip(&shown) {
            o.covered = match self.cover_policy {
                CoverPolicy::Off => false,
                CoverPolicy::ThatMonitor => !visible,
                CoverPolicy::AllMonitors => any_hidden,
            };
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
