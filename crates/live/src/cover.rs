//! Whether app windows fill a screen. On macOS a zoomed window (a double
//! click on its title bar) leaves the menu bar and the Dock showing, so the
//! wallpaper's window never reports itself hidden: the Mac host also asks
//! where the other apps' windows are and treats a screen whose usable area
//! they all but fill (one zoomed window, or tiles side by side) as covered.

use std::time::{Duration, Instant};

/// A rectangle in one coordinate system (the Mac host: points, top-left
/// origin, as the window server gives window bounds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    fn intersects(&self, other: Rect) -> bool {
        self.w > 0.0
            && self.h > 0.0
            && self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    fn covers(&self, other: Rect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.w >= other.x + other.w
            && self.y + self.h >= other.y + other.h
    }
}

/// From this share of a screen's usable area covered, the screen counts as
/// covered: a zoomed or maximized window, or windows tiled side by side
/// (with the small gaps tiling leaves).
pub const FILLED: f64 = 0.92;

/// The share of `area` the windows cover together, sampled on a grid.
pub fn covered_share(area: Rect, windows: &[Rect]) -> f64 {
    const COLS: usize = 64;
    const ROWS: usize = 36;
    if area.w <= 0.0 || area.h <= 0.0 || windows.is_empty() {
        return 0.0;
    }
    let mut local = [Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    }; 32];
    let mut n = 0usize;
    let mut overflow = false;
    for &w in windows {
        if !w.intersects(area) {
            continue;
        }
        if w.covers(area) {
            return 1.0;
        }
        if n < local.len() {
            local[n] = w;
            n += 1;
        } else {
            overflow = true;
        }
    }
    if n == 0 {
        return 0.0;
    }
    let candidates: &[Rect] = if overflow { windows } else { &local[..n] };
    let mut hit = 0usize;
    for r in 0..ROWS {
        let py = area.y + (r as f64 + 0.5) * area.h / ROWS as f64;
        for c in 0..COLS {
            let px = area.x + (c as f64 + 0.5) * area.w / COLS as f64;
            if candidates.iter().any(|w| w.contains(px, py)) {
                hit += 1;
            }
        }
    }
    hit as f64 / (ROWS * COLS) as f64
}

/// Whether the windows all but fill `area`.
pub fn filled(area: Rect, windows: &[Rect]) -> bool {
    covered_share(area, windows) >= FILLED
}

/// From this share covered, the menu bar above the windows is made solid: an
/// almost maximized window too, with some desktop around it.
pub const ALMOST_FILLED: f64 = 0.85;

/// How far below the menu bar a window's rounded corners reach, in points.
pub const CORNERS: f64 = 24.0;

/// What goes behind a screen's menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backing {
    /// Nothing: the wallpaper shows.
    Clear,
    /// One colour behind the menu bar.
    Bar,
    /// And in the corners of the windows right below it.
    BarAndCorners,
}

/// The backing for windows so, at rest, on a screen whose usable part is
/// `area`.
pub fn backing(area: Rect, windows: &[Rect]) -> Backing {
    backing_with_share(area, windows, covered_share(area, windows))
}

fn backing_with_share(area: Rect, windows: &[Rect], share: f64) -> Backing {
    if share < ALMOST_FILLED {
        return Backing::Clear;
    }
    let top = Rect { h: CORNERS, ..area };
    if covered_share(top, windows) >= 0.97 {
        Backing::BarAndCorners
    } else {
        Backing::Bar
    }
}

/// How long windows that don't reach the menu bar must hold still before it
/// turns solid above them.
pub const REST: Duration = Duration::from_millis(80);

/// A menu bar's backing from one look at the windows to the next. It changes
/// as a window reaches the menu bar or starts to leave it (the window server
/// tells where a window is all through its minimizing), not once it is still.
#[derive(Clone, Copy, Debug)]
pub struct Watch {
    backing: Backing,
    /// The share covered at the last look, since when it has been that, and
    /// what it was when the backing went up.
    share: f64,
    since: Option<Instant>,
    rest: f64,
}

impl Default for Watch {
    fn default() -> Watch {
        Watch {
            backing: Backing::Clear,
            share: 0.0,
            since: None,
            rest: 0.0,
        }
    }
}

impl Watch {
    pub fn backing(&self) -> Backing {
        self.backing
    }

    /// The backing now; and whether windows moved since the last look.
    pub fn look(&mut self, now: Instant, area: Rect, windows: &[Rect]) -> (Backing, bool) {
        let share = covered_share(area, windows);
        let moved = (share - self.share).abs() > 1e-6;
        if moved || self.since.is_none() {
            self.since = Some(now);
        }
        self.share = share;
        let still = self.since.is_some_and(|t| now >= t + REST);
        self.backing = match (self.backing, backing_with_share(area, windows, share)) {
            (_, Backing::Clear) => Backing::Clear,
            // A window has the menu bar right above it, or just had.
            (_, Backing::BarAndCorners) => Backing::BarAndCorners,
            (Backing::BarAndCorners, Backing::Bar) => Backing::Clear,
            (Backing::Bar, Backing::Bar) if share >= self.rest - 0.01 => Backing::Bar,
            (Backing::Bar, Backing::Bar) => Backing::Clear,
            (Backing::Clear, Backing::Bar) if still => {
                self.rest = share;
                Backing::Bar
            }
            (Backing::Clear, Backing::Bar) => Backing::Clear,
        };
        (self.backing, moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1512 x 982 point screen: the menu bar is 37 high, the Dock 70.
    const VISIBLE: Rect = Rect {
        x: 0.0,
        y: 37.0,
        w: 1512.0,
        h: 875.0,
    };

    #[test]
    fn a_zoomed_window_fills_the_screen() {
        assert!(filled(VISIBLE, &[VISIBLE]));
        // A few points short of the edges still counts.
        let almost = Rect {
            x: 6.0,
            y: 43.0,
            w: 1500.0,
            h: 860.0,
        };
        assert!(filled(VISIBLE, &[almost]));
    }

    #[test]
    fn a_big_window_with_desktop_around_it_does_not() {
        let big = Rect {
            x: 150.0,
            y: 120.0,
            w: 1200.0,
            h: 700.0,
        };
        assert!(!filled(VISIBLE, &[big]));
        assert!(!filled(VISIBLE, &[]));
    }

    #[test]
    fn the_menu_bar_is_solid_above_windows_that_almost_fill_the_screen() {
        assert_eq!(backing(VISIBLE, &[]), Backing::Clear);
        assert_eq!(backing(VISIBLE, &[VISIBLE]), Backing::BarAndCorners);
        // Some desktop all around: the bar, and not a band below it.
        let almost = Rect {
            x: 30.0,
            y: 60.0,
            w: 1452.0,
            h: 830.0,
        };
        assert!(!filled(VISIBLE, &[almost]));
        assert_eq!(backing(VISIBLE, &[almost]), Backing::Bar);
        // Full width, right under the bar, but only half the height.
        let half = Rect {
            h: 440.0,
            ..VISIBLE
        };
        assert_eq!(backing(VISIBLE, &[half]), Backing::Clear);
    }

    /// A window minimizing and coming back, as macOS 26 reported one (the
    /// screen's usable part is 31 points down), a look every 16 ms.
    #[test]
    fn the_menu_bar_clears_as_a_window_starts_to_minimize() {
        let area = Rect {
            x: 0.0,
            y: 31.0,
            w: 1024.0,
            h: 674.0,
        };
        let at = |x: f64, y: f64, w: f64, h: f64| [Rect { x, y, w, h }];
        let start = Instant::now();
        let ms = |n: u64| start + Duration::from_millis(n);
        let mut watch = Watch::default();
        let full = at(0.0, 31.0, 1024.0, 674.0);
        // Solid at the first look at a window right under it.
        assert_eq!(watch.look(ms(0), area, &full).0, Backing::BarAndCorners);
        assert_eq!(
            watch.look(ms(16), area, &at(0.0, 31.0, 1024.0, 675.0)).0,
            Backing::BarAndCorners
        );
        // Three points down: clear, and it stays so all the way, though the
        // window still almost fills the screen and two looks see the same.
        let away = [
            at(0.0, 34.0, 1024.0, 675.0),
            at(0.0, 34.0, 1024.0, 675.0),
            at(5.0, 64.0, 1019.0, 675.0),
            at(22.0, 99.0, 1001.0, 646.0),
            at(245.0, 267.0, 759.0, 478.0),
        ];
        for (i, w) in away.iter().enumerate() {
            let (backing, _) = watch.look(ms(32 + 16 * i as u64), area, w);
            assert_eq!(backing, Backing::Clear, "look {i}");
        }
        // Coming back: clear until it is there, then solid at once.
        assert_eq!(
            watch.look(ms(300), area, &at(12.0, 81.0, 1011.0, 664.0)).0,
            Backing::Clear
        );
        assert_eq!(
            watch.look(ms(316), area, &at(0.0, 41.0, 1024.0, 675.0)).0,
            Backing::Clear
        );
        assert_eq!(watch.look(ms(332), area, &full).0, Backing::BarAndCorners);
        // Closed: clear.
        assert_eq!(watch.look(ms(348), area, &[]), (Backing::Clear, true));
        assert_eq!(watch.look(ms(364), area, &[]), (Backing::Clear, false));
    }

    #[test]
    fn a_window_with_desktop_around_it_waits_to_hold_still() {
        let start = Instant::now();
        let ms = |n: u64| start + Duration::from_millis(n);
        let almost = Rect {
            x: 30.0,
            y: 60.0,
            w: 1452.0,
            h: 830.0,
        };
        let mut watch = Watch::default();
        assert_eq!(watch.look(ms(0), VISIBLE, &[almost]).0, Backing::Clear);
        assert_eq!(watch.look(ms(16), VISIBLE, &[almost]).0, Backing::Clear);
        assert_eq!(watch.look(ms(96), VISIBLE, &[almost]).0, Backing::Bar);
        // It starts to leave the screen: clear, though it still almost fills.
        let moved = Rect { y: 110.0, ..almost };
        assert_eq!(watch.look(ms(112), VISIBLE, &[moved]).0, Backing::Clear);
    }

    #[test]
    fn two_tiles_side_by_side_fill_it_and_other_screens_do_not_count() {
        let left = Rect {
            x: 8.0,
            y: 45.0,
            w: 744.0,
            h: 859.0,
        };
        let right = Rect {
            x: 760.0,
            y: 45.0,
            w: 744.0,
            h: 859.0,
        };
        assert!(filled(VISIBLE, &[left, right]));
        assert!(!filled(VISIBLE, &[left]));
        // A window on the screen to the right.
        let elsewhere = Rect {
            x: 1512.0,
            y: 0.0,
            w: 2560.0,
            h: 1440.0,
        };
        assert!(!filled(VISIBLE, &[elsewhere]));
    }
}
