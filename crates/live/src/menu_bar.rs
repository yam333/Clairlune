//! A solid menu bar while app windows all but fill a screen (macOS).
//!
//! From macOS 26 the menu bar is clear, and above a zoomed window it is the
//! only strip of wallpaper left: a busy one behind the menus. While windows
//! all but fill a screen ([`cover::backing`]), a window of one colour sits
//! behind its menu bar, just above the wallpaper: dark grey over a dark
//! wallpaper, white over a light one. macOS picks the menus' own colour from
//! the wallpaper, a moment after it changes; the strip goes by what it picked
//! ([`is_dark`]) where that is known, so the menus always read. Earlier
//! systems blur the bar into one colour themselves.
//!
//! Everything here runs on the main thread (AppKit's rule).

use crate::cover::{self, Backing, CORNERS};
use crate::mac::{AppWindows, WINDOW_CHECK, desktop_level, join_desktop, usable_area};
use clairlune_platform::macos::find_screen;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAppearanceCustomization, NSBackingStoreType, NSColor, NSScreen, NSStatusItem, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSPoint, NSProcessInfo, NSRect, NSSize};
use objc2_quartz_core::{CALayer, CATransaction};
use std::time::Instant;

define_class!(
    // SAFETY: NSWindow asks nothing of its subclasses, and this one has no
    // state and no Drop.
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[name = "ClairluneMenuBarWindow"]
    struct BarWindow;

    impl BarWindow {
        /// AppKit moves windows out from under the menu bar; this one's
        /// place is there.
        #[unsafe(method(constrainFrameRect:toScreen:))]
        fn constrain(&self, frame: NSRect, _screen: Option<&NSScreen>) -> NSRect {
            frame
        }
    }
);

/// The strip behind one screen's menu bar.
struct Strip {
    /// The menu bar, and below it as far as a window's rounded corners reach.
    frame: NSRect,
    layer: Retained<CALayer>,
    window: Retained<BarWindow>,
}

struct Bar {
    /// The monitor's id, and whether its wallpaper is dark.
    monitor: String,
    dark: bool,
    /// What is behind the menu bar, and whether that is dark.
    backing: Backing,
    shown_dark: bool,
    strip: Option<Strip>,
}

/// The menu bars of the screens with a wallpaper of ours.
#[derive(Default)]
pub struct MenuBars {
    bars: Vec<Bar>,
    /// Whether macOS takes the menu bar for a dark one, when known.
    seen_dark: Option<bool>,
    checked: Option<Instant>,
}

/// Whether the menu bar is clear here. An app built for an older system is
/// told that macOS 26 is 16.
fn clear_menu_bar() -> bool {
    NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion
        >= 16
}

/// Whether macOS takes the menu bar for a dark one (its menus are white),
/// from how it draws an item there.
pub fn is_dark(item: &NSStatusItem) -> Option<bool> {
    let button = item.button(MainThreadMarker::new()?)?;
    Some(
        button
            .effectiveAppearance()
            .name()
            .to_string()
            .contains("Dark"),
    )
}

impl MenuBars {
    /// The screens with a wallpaper of ours: the monitor's id, and whether
    /// the wallpaper is dark.
    pub fn set(&mut self, screens: Vec<(String, bool)>) {
        if !clear_menu_bar() {
            return;
        }
        let same = screens.len() == self.bars.len()
            && screens
                .iter()
                .zip(&self.bars)
                .all(|((id, _), b)| *id == b.monitor);
        if same {
            for ((_, dark), bar) in screens.into_iter().zip(&mut self.bars) {
                bar.dark = dark;
            }
        } else {
            self.bars = screens
                .into_iter()
                .map(|(monitor, dark)| Bar {
                    monitor,
                    dark,
                    backing: Backing::Clear,
                    shown_dark: dark,
                    strip: None,
                })
                .collect();
        }
        self.checked = None;
    }

    /// Whether [`MenuBars::tick`] would look now, so wants to be told of the
    /// menu bar first.
    pub fn due(&self, now: Instant) -> bool {
        !self.bars.is_empty() && self.checked.is_none_or(|t| now >= t + WINDOW_CHECK)
    }

    /// Whether macOS takes the menu bar for a dark one ([`is_dark`]).
    pub fn see(&mut self, dark: Option<bool>) {
        self.seen_dark = dark;
    }

    /// Looks where the windows are when that's due; returns when to come
    /// back.
    pub fn tick(&mut self, now: Instant, windows: &mut AppWindows) -> Option<Instant> {
        if self.bars.is_empty() {
            return None;
        }
        if self.due(now) {
            self.checked = Some(now);
            if let Some(mtm) = MainThreadMarker::new() {
                let windows = windows.look();
                for bar in &mut self.bars {
                    bar.update(mtm, windows, self.seen_dark);
                }
            }
        }
        self.checked.map(|t| t + WINDOW_CHECK)
    }
}

impl Bar {
    fn update(&mut self, mtm: MainThreadMarker, windows: &[cover::Rect], seen: Option<bool>) {
        let screen = find_screen(mtm, &self.monitor);
        let place = screen.as_ref().and_then(|s| strip_frame(s));
        // Another resolution, or the menu bar or the screen went.
        if self.strip.as_ref().is_some_and(|s| Some(s.frame) != place) {
            self.strip = None;
            self.backing = Backing::Clear;
        }
        let (Some(screen), Some(frame)) = (screen, place) else {
            return;
        };
        let backing =
            usable_area(mtm, &screen).map_or(Backing::Clear, |a| cover::backing(a, windows));
        let dark = if backing == Backing::Clear {
            self.shown_dark
        } else {
            seen.unwrap_or(self.dark)
        };
        if (backing, dark) == (self.backing, self.shown_dark) {
            return;
        }
        let solid = backing != Backing::Clear;
        if solid != (self.backing != Backing::Clear) || dark != self.shown_dark {
            tracing::info!(target: "clairlune", screen = %self.monitor, solid, dark, "menu bar");
        }
        (self.backing, self.shown_dark) = (backing, dark);
        self.strip
            .get_or_insert_with(|| Strip::new(mtm, frame, dark))
            .show(backing, dark);
    }
}

/// Where the strip goes; none on a screen without a menu bar.
fn strip_frame(screen: &NSScreen) -> Option<NSRect> {
    let (f, v) = (screen.frame(), screen.visibleFrame());
    let bottom = v.origin.y + v.size.height;
    let height = f.origin.y + f.size.height - bottom;
    (height >= 1.0).then(|| {
        NSRect::new(
            NSPoint::new(f.origin.x, bottom - CORNERS),
            NSSize::new(f.size.width, height + CORNERS),
        )
    })
}

fn colour(dark: bool) -> CFRetained<CGColor> {
    let (r, g, b) = if dark {
        (0.157, 0.157, 0.165)
    } else {
        (1.0, 1.0, 1.0)
    };
    CGColor::new_srgb(r, g, b, 1.0)
}

impl Strip {
    fn new(mtm: MainThreadMarker, frame: NSRect, dark: bool) -> Strip {
        let this = BarWindow::alloc(mtm).set_ivars(());
        // SAFETY: NSWindow's own initialiser.
        let window: Retained<BarWindow> = unsafe {
            msg_send![
                super(this),
                initWithContentRect: frame,
                styleMask: NSWindowStyleMask::Borderless,
                backing: NSBackingStoreType::Buffered,
                defer: false,
            ]
        };
        // Above the wallpaper (ours, live, is at the desktop's own level) and
        // below the desktop icons.
        join_desktop(&window, desktop_level() + 1);
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        let view =
            NSView::initWithFrame(NSView::alloc(mtm), NSRect::new(NSPoint::ZERO, frame.size));
        let root = CALayer::new();
        view.setLayer(Some(&root));
        view.setWantsLayer(true);
        let layer = CALayer::new();
        layer.setBackgroundColor(Some(&colour(dark)));
        layer.setOpacity(0.0);
        root.addSublayer(&layer);
        window.setContentView(Some(&view));
        window.setFrame_display(frame, false);
        window.orderFrontRegardless();
        // Clear first, so that showing it fades it in.
        CATransaction::flush();
        Strip {
            frame,
            layer,
            window,
        }
    }

    /// Fades to the backing (Core Animation's own quarter of a second).
    fn show(&self, backing: Backing, dark: bool) {
        let size = self.frame.size;
        let below = match backing {
            Backing::Clear => {
                self.layer.setOpacity(0.0);
                return;
            }
            Backing::Bar => CORNERS,
            Backing::BarAndCorners => 0.0,
        };
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.layer.setFrame(NSRect::new(
            NSPoint::new(0.0, below),
            NSSize::new(size.width, size.height - below),
        ));
        CATransaction::commit();
        self.layer.setBackgroundColor(Some(&colour(dark)));
        self.layer.setOpacity(1.0);
    }
}

impl Drop for Strip {
    fn drop(&mut self) {
        self.window.orderOut(None);
        self.window.close();
    }
}
