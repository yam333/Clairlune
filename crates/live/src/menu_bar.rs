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
//! It comes and goes with the windows, in one frame. Nothing tells an app of
//! other apps' windows without a permission, so it looks: twice a second, and
//! [`FAST`] for a moment after a click or a modifier key (what minimizing,
//! zooming, closing and switching start with) and while windows move.
//!
//! Everything here runs on the main thread (AppKit's rule).

use crate::cover::{Backing, CORNERS, Rect, Watch};
use crate::mac::{AppWindows, WINDOW_CHECK, desktop_level, join_desktop, usable_area};
use clairlune_platform::macos::find_screen;
use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAppearanceCustomization, NSBackingStoreType, NSColor, NSScreen, NSStatusItem, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{CGColor, CGEventSource, CGEventSourceStateID, CGEventType};
use objc2_foundation::{NSActivityOptions, NSPoint, NSProcessInfo, NSRect, NSSize, ns_string};
use objc2_quartz_core::{CALayer, CATransaction};
use std::time::{Duration, Instant};

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
    /// The screen's usable part, and where the strip goes: none on a screen
    /// without a menu bar.
    place: Option<(Rect, NSRect)>,
    /// What is behind the menu bar, and whether that is dark.
    watch: Watch,
    shown: (Backing, bool),
    strip: Option<Strip>,
}

/// How often to look while something may be moving: one 60 Hz frame.
const FAST: Duration = Duration::from_millis(16);
/// For how long after a click, a modifier key or an app change, and after
/// windows moved.
const FAST_FOR: Duration = Duration::from_millis(550);
/// How often to look for new windows too, meanwhile.
const FIND: Duration = Duration::from_millis(80);
/// How often to re-measure screen and Dock frames when nothing notified us.
const MEASURE_CHECK: Duration = Duration::from_secs(5);
/// How often to ask whether there was a click or a modifier key while active
/// (backing off to [`WINDOW_CHECK`] once idle).
const INPUT_ACTIVE: Duration = Duration::from_millis(100);
const INPUT_WARM: Duration = Duration::from_millis(250);

/// How many clicks and modifier key presses there were (excluding scroll/trackpad
/// gestures and drags, so scrolling a page doesn't spin up window polling).
/// Counting them needs no permission; which they were isn't known.
fn input_count() -> u32 {
    [
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGEventType::FlagsChanged,
    ]
    .into_iter()
    .map(|t| CGEventSource::counter_for_event_type(CGEventSourceStateID::CombinedSessionState, t))
    .fold(0, u32::wrapping_add)
}

/// The menu bars of the screens with a wallpaper of ours.
#[derive(Default)]
pub struct MenuBars {
    bars: Vec<Bar>,
    /// Whether macOS takes the menu bar for a dark one, when known.
    seen_dark: Option<bool>,
    seen_at: Option<Instant>,
    /// When the windows, the screens and the input were last looked at.
    checked: Option<Instant>,
    measured: Option<Instant>,
    found: Option<Instant>,
    asked_input: Option<Instant>,
    last_input: Option<Instant>,
    input: u32,
    fast_until: Option<Instant>,
    /// Held while a menu bar is solid: macOS slows down (App Nap) an app
    /// whose wallpaper nobody sees, and then it would clear late.
    awake: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
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
                    place: None,
                    watch: Watch::default(),
                    shown: (Backing::Clear, dark),
                    strip: None,
                })
                .collect();
        }
        self.checked = None;
        self.measured = None;
        self.seen_at = None;
        self.stay_awake();
    }

    /// Whether [`MenuBars::see`] wants a fresh look at the menu bar's text
    /// appearance (twice a second at most, not every animation frame).
    pub fn appearance_due(&self, now: Instant) -> bool {
        !self.bars.is_empty()
            && !clairlune_platform::macos::screens_asleep()
            && self.seen_at.is_none_or(|t| now >= t + WINDOW_CHECK)
    }

    /// Whether [`MenuBars::tick`] would look now.
    pub fn due(&self, now: Instant) -> bool {
        !self.bars.is_empty() && self.checked.is_none_or(|t| now >= t + self.pace(now))
    }

    /// Notes that an app activated, hid or unhid, or Spaces/screens changed,
    /// so windows are re-checked right away.
    pub fn note_windows_changed(&mut self, now: Instant) {
        if self.bars.is_empty() {
            return;
        }
        self.checked = None;
        self.measured = None;
        self.found = None;
        self.last_input = Some(now);
        self.fast_until = Some(now + FAST_FOR);
    }

    /// How long from one look at the windows to the next.
    fn pace(&self, now: Instant) -> Duration {
        match self.fast_until {
            Some(t) if now < t => FAST,
            _ => WINDOW_CHECK,
        }
    }

    /// How often to check click/modifier counters: brisk right after input,
    /// settling to [`WINDOW_CHECK`] once the Mac has been untouched for 30 s.
    fn input_pace(&self, now: Instant) -> Duration {
        let Some(last) = self.last_input else {
            return WINDOW_CHECK;
        };
        let idle = now.saturating_duration_since(last);
        if idle < Duration::from_secs(5) {
            INPUT_ACTIVE
        } else if idle < Duration::from_secs(30) {
            INPUT_WARM
        } else {
            WINDOW_CHECK
        }
    }

    /// Whether macOS takes the menu bar for a dark one ([`is_dark`]).
    pub fn see(&mut self, now: Instant, dark: Option<bool>) {
        self.seen_at = Some(now);
        self.seen_dark = dark;
    }

    /// Looks where the windows are when that's due; returns when to come
    /// back.
    pub fn tick(&mut self, now: Instant, windows: &mut AppWindows) -> Option<Instant> {
        if self.bars.is_empty() || clairlune_platform::macos::screens_asleep() {
            return None;
        }
        let ask_every = self.input_pace(now);
        if self.asked_input.is_none_or(|t| now >= t + ask_every) {
            self.asked_input = Some(now);
            let input = input_count();
            let prev = std::mem::replace(&mut self.input, input);
            if prev != input {
                self.last_input = Some(now);
                self.fast_until = Some(now + FAST_FOR);
            }
        }
        if self.due(now)
            && let Some(mtm) = MainThreadMarker::new()
        {
            self.checked = Some(now);
            if self.measured.is_none_or(|t| now >= t + MEASURE_CHECK) {
                self.measured = Some(now);
                for bar in &mut self.bars {
                    bar.measure(mtm);
                }
            }
            let windows = if self.found.is_none_or(|t| now >= t + FIND) {
                self.found = Some(now);
                windows.look_now()
            } else {
                windows.look_again()
            };
            let mut moved = false;
            for bar in &mut self.bars {
                moved |= bar.update(mtm, now, windows, self.seen_dark);
            }
            if moved {
                self.fast_until = Some(now + FAST_FOR);
            }
            self.stay_awake();
        }
        let look = self.checked.map(|t| t + self.pace(now));
        let ask = self.asked_input.map(|t| t + self.input_pace(now));
        [look, ask].into_iter().flatten().min()
    }

    fn stay_awake(&mut self) {
        let solid = self.bars.iter().any(|b| b.shown.0 != Backing::Clear);
        if solid == self.awake.is_some() {
            return;
        }
        let process = NSProcessInfo::processInfo();
        match self.awake.take() {
            // SAFETY: what beginActivity gave.
            Some(activity) => unsafe { process.endActivity(&activity) },
            None => {
                self.awake = Some(process.beginActivityWithOptions_reason(
                    NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
                    ns_string!("The menu bar follows the windows"),
                ));
            }
        }
    }
}

impl Bar {
    /// Where the screen and its menu bar are.
    fn measure(&mut self, mtm: MainThreadMarker) {
        self.place = find_screen(mtm, &self.monitor)
            .and_then(|s| Some((usable_area(mtm, &s)?, strip_frame(&s)?)));
        // Another resolution, or the menu bar or the screen went.
        let frame = self.place.map(|(_, f)| f);
        if self.strip.as_ref().is_some_and(|s| Some(s.frame) != frame) {
            self.strip = None;
            self.watch = Watch::default();
            self.shown.0 = Backing::Clear;
        }
    }

    /// Looks, and shows what it finds; whether windows moved.
    fn update(
        &mut self,
        mtm: MainThreadMarker,
        now: Instant,
        windows: &[Rect],
        seen: Option<bool>,
    ) -> bool {
        let Some((area, frame)) = self.place else {
            return false;
        };
        let (backing, moved) = self.watch.look(now, area, windows);
        let solid = backing != Backing::Clear;
        let dark = if solid {
            seen.unwrap_or(self.dark)
        } else {
            self.shown.1
        };
        if (backing, dark) != self.shown {
            if solid != (self.shown.0 != Backing::Clear) || dark != self.shown.1 {
                tracing::info!(target: "clairlune", screen = %self.monitor, solid, dark, "menu bar");
            }
            self.shown = (backing, dark);
            self.strip
                .get_or_insert_with(|| Strip::new(mtm, frame, dark))
                .show(backing, dark);
        }
        moved
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
        Strip {
            frame,
            layer,
            window,
        }
    }

    /// Shows the backing, in the next frame.
    fn show(&self, backing: Backing, dark: bool) {
        let size = self.frame.size;
        let below = match backing {
            Backing::BarAndCorners => 0.0,
            Backing::Bar | Backing::Clear => CORNERS,
        };
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.layer.setFrame(NSRect::new(
            NSPoint::new(0.0, below),
            NSSize::new(size.width, size.height - below),
        ));
        self.layer.setBackgroundColor(Some(&colour(dark)));
        self.layer
            .setOpacity(if backing == Backing::Clear { 0.0 } else { 1.0 });
        CATransaction::commit();
        CATransaction::flush();
    }
}

impl Drop for Strip {
    fn drop(&mut self) {
        self.window.orderOut(None);
        self.window.close();
    }
}
