//! Whether app windows fill a screen. On macOS a zoomed window (a double
//! click on its title bar) leaves the menu bar and the Dock showing, so the
//! wallpaper's window never reports itself hidden: the Mac host also asks
//! where the other apps' windows are and treats a screen whose usable area
//! they all but fill (one zoomed window, or tiles side by side) as covered.

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
}

/// From this share of a screen's usable area covered, the screen counts as
/// covered: a zoomed or maximized window, or windows tiled side by side
/// (with the small gaps tiling leaves).
pub const FILLED: f64 = 0.92;

/// The share of `area` the windows cover together, sampled on a grid.
pub fn covered_share(area: Rect, windows: &[Rect]) -> f64 {
    const COLS: usize = 64;
    const ROWS: usize = 36;
    if area.w <= 0.0 || area.h <= 0.0 {
        return 0.0;
    }
    let mut hit = 0usize;
    for r in 0..ROWS {
        let py = area.y + (r as f64 + 0.5) * area.h / ROWS as f64;
        for c in 0..COLS {
            let px = area.x + (c as f64 + 0.5) * area.w / COLS as f64;
            if windows.iter().any(|w| w.contains(px, py)) {
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

/// The backing for windows so, on a screen whose usable part is `area`.
pub fn backing(area: Rect, windows: &[Rect]) -> Backing {
    if covered_share(area, windows) < ALMOST_FILLED {
        return Backing::Clear;
    }
    let top = Rect { h: CORNERS, ..area };
    if covered_share(top, windows) >= 0.97 {
        Backing::BarAndCorners
    } else {
        Backing::Bar
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
