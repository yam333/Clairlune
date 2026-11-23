//! One scene spread across every monitor: where each monitor sits, which
//! one the scene centres on, and the part of the scene each shows.
//!
//! Monitors are placed in the system's display arrangement, in its own
//! units (points on macOS, pixels on Windows), and moved where Settings has
//! put them. The scene is drawn at the lowest density among the monitors
//! (physical pixels per unit): a 1x monitor shows its part pixel for pixel,
//! a Retina one beside it shows its part pixel-doubled, and the picture
//! lines up across them. A gap left between two monitors stays in the scene,
//! as the frames between real screens do.

use crate::settings::DisplaySettings;
use clairlune_platform::Monitor;
use clairlune_render::PixelRect;

/// A monitor where it sits for the spread, in arrangement units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// Physical pixels per unit (2 on a Retina screen).
    pub scale: u32,
    /// The scene centres on it.
    pub main: bool,
}

impl Placed {
    fn right(&self) -> i32 {
        self.x + self.w as i32
    }

    fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }
}

/// The monitors placed: where Settings has put them, else where the system
/// does. The main one is Settings' pick while it's connected, else the
/// system's main monitor.
pub fn arrange(monitors: &[Monitor], display: &DisplaySettings) -> Vec<Placed> {
    let chosen = display
        .span_main
        .as_deref()
        .filter(|id| monitors.iter().any(|m| m.id == *id));
    let mut placed: Vec<Placed> = monitors
        .iter()
        .map(|m| {
            let (w, h) = m.arranged_size();
            let [x, y] = display
                .monitors
                .iter()
                .find(|s| s.id == m.id)
                .and_then(|s| s.span_at)
                .unwrap_or([m.x, m.y]);
            Placed {
                id: m.id.clone(),
                x,
                y,
                w,
                h,
                scale: m.scale.max(1),
                main: match chosen {
                    Some(id) => m.id == id,
                    None => m.primary,
                },
            }
        })
        .collect();
    if !placed.iter().any(|p| p.main)
        && let Some(first) = placed.first_mut()
    {
        first.main = true;
    }
    placed
}

/// The scene the monitors share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spread {
    /// The scene's size in pixels: every monitor's place, gaps included.
    pub width: u32,
    pub height: u32,
    /// The part each monitor shows, in the order they were placed.
    pub views: Vec<PixelRect>,
    /// The main monitor's middle, in thousandths of the scene's width.
    pub focus_pm: u32,
    /// Scene pixels per arrangement unit: the lowest density of them all.
    pub density: u32,
}

/// The scene for monitors placed so.
pub fn spread(placed: &[Placed]) -> Spread {
    let density = placed.iter().map(|p| p.scale).min().unwrap_or(1).max(1);
    let x0 = placed.iter().map(|p| p.x).min().unwrap_or(0);
    let y0 = placed.iter().map(|p| p.y).min().unwrap_or(0);
    let x1 = placed.iter().map(Placed::right).max().unwrap_or(0);
    let y1 = placed.iter().map(Placed::bottom).max().unwrap_or(0);
    let views: Vec<PixelRect> = placed
        .iter()
        .map(|p| PixelRect {
            x: (p.x - x0) as u32 * density,
            y: (p.y - y0) as u32 * density,
            w: p.w * density,
            h: p.h * density,
        })
        .collect();
    let width = ((x1 - x0) as u32 * density).max(1);
    let height = ((y1 - y0) as u32 * density).max(1);
    let main = placed.iter().position(|p| p.main).unwrap_or(0);
    let focus_pm = views.get(main).map_or(500, |v| {
        (u64::from(v.x + v.w / 2) * 1000 / u64::from(width)) as u32
    });
    Spread {
        width,
        height,
        views,
        focus_pm,
        density,
    }
}

fn overlaps(a: (i32, i32, i32, i32), b: &Placed) -> bool {
    let (x, y, w, h) = a;
    x < b.right() && b.x < x + w && y < b.bottom() && b.y < y + h
}

/// Where monitor `i`, dragged to (`x`, `y`), lands: within `reach` units of
/// touching or lining up with another monitor's edge, it snaps there; and it
/// never ends up over another one (it's pushed out the shortest way).
pub fn snap(placed: &[Placed], i: usize, x: i32, y: i32, reach: i32) -> (i32, i32) {
    let me = &placed[i];
    let (w, h) = (me.w as i32, me.h as i32);
    let others: Vec<&Placed> = placed
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(_, p)| p)
        .collect();
    let nearest = |at: i32, candidates: &mut dyn Iterator<Item = i32>| {
        candidates
            .map(|c| ((c - at).abs(), c))
            .filter(|(d, _)| *d <= reach)
            .min()
            .map_or(at, |(_, c)| c)
    };
    // Beside another (touching), or lined up with its edges or middle.
    let mut xs = others.iter().flat_map(|o| {
        [
            o.x - w,
            o.right(),
            o.x,
            o.right() - w,
            o.x + (o.w as i32 - w) / 2,
        ]
    });
    let mut ys = others.iter().flat_map(|o| {
        [
            o.y - h,
            o.bottom(),
            o.y,
            o.bottom() - h,
            o.y + (o.h as i32 - h) / 2,
        ]
    });
    let (mut nx, mut ny) = (nearest(x, &mut xs), nearest(y, &mut ys));
    for _ in 0..2 * others.len().max(1) {
        let Some(o) = others.iter().find(|o| overlaps((nx, ny, w, h), o)) else {
            break;
        };
        let ways = [
            (o.x - w, ny),
            (o.right(), ny),
            (nx, o.y - h),
            (nx, o.bottom()),
        ];
        (nx, ny) = ways
            .into_iter()
            .min_by_key(|(ax, ay)| (ax - nx).abs() + (ay - ny).abs())
            .unwrap_or((nx, ny));
    }
    (nx, ny)
}

/// What the spread depends on in Settings, to tell when it changed.
pub fn settings_key(display: &DisplaySettings) -> String {
    let mut placed: Vec<String> = display
        .monitors
        .iter()
        .filter_map(|m| m.span_at.map(|[x, y]| format!("{}@{x},{y}", m.id)))
        .collect();
    placed.sort();
    format!(
        "{}|{}|{}",
        display.span,
        display.span_main.as_deref().unwrap_or(""),
        placed.join(";")
    )
}

/// Whether Settings has its own arrangement or main monitor.
pub fn customised(display: &DisplaySettings) -> bool {
    display.span_main.is_some() || display.monitors.iter().any(|m| m.span_at.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: &str, x: i32, y: i32, w: u32, h: u32, scale: u32, primary: bool) -> Monitor {
        Monitor {
            id: id.into(),
            x,
            y,
            width: w,
            height: h,
            primary,
            scale,
        }
    }

    /// An external main monitor, another to its right, and a Retina laptop
    /// under the main one (macOS: positions in points).
    fn desk() -> Vec<Monitor> {
        vec![
            mon("main", 0, 0, 2560, 1440, 1, true),
            mon("right", 2560, 0, 1920, 1080, 1, false),
            mon("laptop", 524, 1440, 3024, 1964, 2, false),
        ]
    }

    #[test]
    fn a_retina_laptop_beside_1x_monitors_lines_up() {
        let placed = arrange(&desk(), &DisplaySettings::default());
        let s = spread(&placed);
        // Drawn at the 1x monitors' density; the laptop's part is half its
        // pixels, shown doubled.
        assert_eq!(s.density, 1);
        assert_eq!((s.width, s.height), (4480, 1440 + 982));
        assert_eq!(
            s.views[0],
            PixelRect {
                x: 0,
                y: 0,
                w: 2560,
                h: 1440
            }
        );
        // The right monitor starts where the main one ends: no gap.
        assert_eq!(
            s.views[1],
            PixelRect {
                x: 2560,
                y: 0,
                w: 1920,
                h: 1080
            }
        );
        assert_eq!(
            s.views[2],
            PixelRect {
                x: 524,
                y: 1440,
                w: 1512,
                h: 982
            }
        );
        // The scene centres on the main monitor.
        assert_eq!(s.focus_pm, 1280 * 1000 / 4480);
    }

    #[test]
    fn retina_screens_alone_are_drawn_at_their_own_density() {
        let two = [
            mon("a", 0, 0, 3024, 1964, 2, true),
            mon("b", 1512, 0, 3840, 2160, 2, false),
        ];
        let s = spread(&arrange(&two, &DisplaySettings::default()));
        assert_eq!(s.density, 2);
        assert_eq!(
            s.views[0],
            PixelRect {
                x: 0,
                y: 0,
                w: 3024,
                h: 1964
            }
        );
        assert_eq!(
            s.views[1],
            PixelRect {
                x: 3024,
                y: 0,
                w: 3840,
                h: 2160
            }
        );
    }

    #[test]
    fn settings_move_monitors_and_pick_the_main_one() {
        let mut s = crate::settings::Settings::default();
        s.edit_monitor("laptop", "Laptop", |m| m.span_at = Some([2560, 1080]));
        let mut display = DisplaySettings {
            span_main: Some("right".into()),
            ..s.display
        };
        let placed = arrange(&desk(), &display);
        assert_eq!((placed[2].x, placed[2].y), (2560, 1080));
        assert!(placed[1].main && !placed[0].main);
        let sp = spread(&placed);
        assert_eq!(sp.focus_pm, (2560 + 960) * 1000 / sp.width);
        // A main monitor that isn't connected: the system's main again.
        display.span_main = Some("gone".into());
        assert!(arrange(&desk(), &display)[0].main);
    }

    #[test]
    fn a_dragged_monitor_snaps_to_edges_and_never_overlaps() {
        let placed = arrange(&desk(), &DisplaySettings::default());
        // Dropped a little off the main monitor's right edge, a little low:
        // it snaps to touch it and lines up with its top.
        assert_eq!(snap(&placed, 1, 2590, 25, 60), (2560, 0));
        // Far from everything: it stays where it was dropped (a gap, kept).
        assert_eq!(snap(&placed, 1, 2900, 600, 60), (2900, 600));
        // Dropped onto the main monitor: pushed out the shortest way.
        let (x, y) = snap(&placed, 2, 200, 1200, 10);
        let me = &placed[2];
        assert!(
            !placed
                .iter()
                .enumerate()
                .any(|(j, o)| j != 2 && overlaps((x, y, me.w as i32, me.h as i32), o)),
            "({x}, {y}) overlaps"
        );
    }

    #[test]
    fn the_key_changes_with_what_the_spread_depends_on() {
        let mut s = crate::settings::Settings::default();
        let a = settings_key(&s.display);
        s.display.span = true;
        let b = settings_key(&s.display);
        s.edit_monitor("x", "X", |m| m.span_at = Some([1, 2]));
        let c = settings_key(&s.display);
        assert!(a != b && b != c);
        assert!(customised(&s.display));
        // A monitor whose only setting is its place is kept.
        assert_eq!(s.display.monitors.len(), 1);
    }
}
