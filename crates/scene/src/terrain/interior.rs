//! Interior: a cozy room. Patterned wallpaper, a window on the side away
//! from the focal group with a landscape outside (another archetype, painted
//! off-screen and seen through the frame, its horizon low in the window),
//! curtains, a shelf of books and a picture on the wall, and a wooden floor
//! for the focal group to stand on (docs/dev/activity-system.md section 2: every
//! outdoor archetype becomes a view from a window).

use super::layers::{put, row_at};
use super::{Ctx, shade_u8};
use crate::glyphs::{EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};

/// Landscapes the window can look out on.
const VIEWS: [&str; 7] = [
    "mountains",
    "forest",
    "shore",
    "desert",
    "snowfield",
    "city_rooftops",
    "canyon",
];

fn pick(seed: u64, salt: i64) -> f64 {
    unit_from_hash(hash3(seed, salt, 0x1D))
}

/// The window, in cells: columns `c0..c1`, rows `r0..r1` (inner glass).
struct Window {
    c0: i64,
    c1: i64,
    r0: i64,
    r1: i64,
}

fn window(ctx: &Ctx, seed: u64) -> Window {
    let group_left = ctx.group_cx < ctx.co.w * 0.5;
    let width = 0.30 + 0.10 * pick(seed, 1);
    let left = if group_left {
        0.52 + (0.42 - width) * pick(seed, 2)
    } else {
        0.06 + (0.42 - width) * pick(seed, 2)
    };
    let top = 0.08 + 0.05 * pick(seed, 3);
    let bottom = (ctx.horizon - 0.14).max(top + 0.22);
    Window {
        c0: ctx.co.col_of(left * ctx.co.w),
        c1: ctx.co.col_of((left + width) * ctx.co.w),
        r0: row_at(ctx, top),
        r1: row_at(ctx, bottom),
    }
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let seed = hash_label(ctx.terrain, "interior");
    let (cols, platform) = (i64::from(ctx.cols), i64::from(ctx.platform_row));
    // Wallpaper: a faint diamond pattern over the whole wall, so the sky
    // pass finds no empty cell outside the window.
    for row in 0..platform {
        for col in 0..cols {
            let on = (col + row * 2) % 6 == 0 || (col - row * 2).rem_euclid(6) == 0;
            let glyph = if on { id('.') } else { id('`') };
            let shade = if on { 0.16 } else { 0.07 };
            put(ctx, cells, col, row, glyph, shade);
        }
    }
    // A skirting board along the bottom of the wall.
    for col in 0..cols {
        put(ctx, cells, col, platform - 1, id('_'), 0.35);
    }

    // The view: an outdoor scene painted off-screen, copied into the window
    // with its horizon 70 % of the way down the glass.
    let w = window(ctx, seed);
    let outdoor = VIEWS[(pick(seed, 4) * VIEWS.len() as f64) as usize % VIEWS.len()];
    let mut view = vec![Cell::default(); cells.len()];
    let _ = super::background(outdoor, ctx, &mut view);
    // The view's sky objects aren't this room's: forget their shelter.
    ctx.unsheltered.borrow_mut().fill(None);
    let horizon_row = i64::from(ctx.platform_row);
    let target = w.r0 + (w.r1 - w.r0) * 7 / 10;
    let dy = horizon_row - target;
    for row in w.r0..w.r1 {
        for col in w.c0..w.c1 {
            let src = row + dy;
            if !(0..i64::from(ctx.rows)).contains(&src) || !(0..cols).contains(&col) {
                continue;
            }
            // A little brighter than outside: the view is what the eye goes to.
            let mut cell = view[ctx.idx(col as u32, src as u32)];
            cell.shade = (u16::from(cell.shade) * 4 / 3).min(255) as u8;
            cells[ctx.idx(col as u32, row as u32)] = cell;
        }
    }

    // The frame: a border, a cross bar, a sill, curtains at both sides.
    let frame = 0.7;
    for col in w.c0 - 1..=w.c1 {
        put(ctx, cells, col, w.r0 - 1, id('='), frame);
        put(ctx, cells, col, w.r1, id('='), frame);
    }
    for row in w.r0..w.r1 {
        put(ctx, cells, w.c0 - 1, row, id('|'), frame);
        put(ctx, cells, w.c1, row, id('|'), frame);
    }
    let mid_c = (w.c0 + w.c1) / 2;
    let mid_r = (w.r0 + w.r1) / 2;
    for row in w.r0..w.r1 {
        put(ctx, cells, mid_c, row, id('|'), 0.4);
    }
    for col in w.c0..w.c1 {
        put(ctx, cells, col, mid_r, id('-'), 0.4);
    }
    for col in w.c0 - 2..=w.c1 + 1 {
        put(ctx, cells, col, w.r1 + 1, id('='), 0.45);
    }
    for (side, x) in [(-1, w.c0 - 2), (1, w.c1 + 1)] {
        for row in w.r0 - 2..w.r1 + 2 {
            for k in 0..3 {
                let fold = if (row + k).rem_euclid(3) == 0 {
                    id('&')
                } else {
                    id('%')
                };
                let glyph = if k == 2 { id(':') } else { fold };
                put(ctx, cells, x + side * k, row, glyph, 0.42 - 0.08 * k as f64);
            }
        }
    }

    // A shelf of books and a picture on the wall, clear of the window and
    // the focal group.
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - 4;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + 4;
    let free = |c0: i64, c1: i64| (c1 < w.c0 - 4 || c0 > w.c1 + 4) && (c1 < g0 || c0 > g1);
    let shelf_c = (pick(seed, 5) * (cols as f64 - 16.0)) as i64 + 2;
    let shelf_r = w.r0 + (w.r1 - w.r0) / 3;
    if free(shelf_c, shelf_c + 11) {
        for col in shelf_c..shelf_c + 12 {
            put(ctx, cells, col, shelf_r, id('='), 0.5);
            // Books of different heights on it.
            let h = 1 + (pick(seed, 100 + col) * 2.5) as i64;
            for dr in 1..=h {
                let glyph = if pick(seed, 200 + col) < 0.2 {
                    EMPTY
                } else {
                    id('|')
                };
                put(ctx, cells, col, shelf_r - dr, glyph, 0.42);
            }
        }
    }
    let pic_c = (pick(seed, 6) * (cols as f64 - 12.0)) as i64 + 2;
    let pic_r = w.r0;
    if free(pic_c, pic_c + 7) && !(shelf_c - 2..shelf_c + 14).contains(&pic_c) {
        for col in pic_c..pic_c + 8 {
            put(ctx, cells, col, pic_r, id('-'), 0.45);
            put(ctx, cells, col, pic_r + 4, id('-'), 0.45);
        }
        for row in pic_r + 1..pic_r + 4 {
            put(ctx, cells, pic_c, row, id('|'), 0.45);
            put(ctx, cells, pic_c + 7, row, id('|'), 0.45);
            for col in pic_c + 1..pic_c + 7 {
                let glyph = if (row + col) % 3 == 0 {
                    id('^')
                } else {
                    id('~')
                };
                put(ctx, cells, col, row, glyph, 0.3);
            }
        }
    }

    // Stars and daytime dust only through the window: everywhere else the
    // sky limit is the top of the screen.
    let mut sky = vec![0.0; ctx.cols as usize];
    let bottom =
        (f64::from(ctx.co.g.origin_y) + w.r1 as f64 * f64::from(ctx.co.g.cell_h)) / ctx.co.h;
    for col in w.c0.max(0)..w.c1.min(cols) {
        sky[col as usize] = bottom;
    }
    sky
}

/// Wooden floorboards across the room: the top row is the surface the feet
/// stand on, then planks with their seams, and a rug under the focal group.
pub(crate) fn floor(ctx: &Ctx, cells: &mut [Cell]) {
    let (cols, top) = (i64::from(ctx.cols), i64::from(ctx.platform_row));
    let seed = hash_label(ctx.terrain, "floor");
    for col in 0..cols {
        put(ctx, cells, col, top, id('='), 0.8);
    }
    for row in top + 1..i64::from(ctx.rows) {
        let offset = (pick(seed, row) * 9.0) as i64;
        for col in 0..cols {
            let seam = (col + offset) % 9 == 0;
            let glyph = if seam {
                id('|')
            } else if (col + row) % 4 == 0 {
                id('-')
            } else {
                id('_')
            };
            let i = ctx.idx(col as u32, row as u32);
            cells[i] = Cell {
                glyph,
                shade: shade_u8(if seam { 0.32 } else { 0.18 }),
            };
        }
    }
    // The rug: a patterned band a little wider than the group, with fringes.
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - 4;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + 4;
    for row in top + 1..(top + 4).min(i64::from(ctx.rows)) {
        for col in g0.max(0)..=g1.min(cols - 1) {
            let edge = col == g0 || col == g1;
            let glyph = if edge {
                id(':')
            } else if (col + row * 2) % 4 == 0 {
                id('#')
            } else {
                id('=')
            };
            let i = ctx.idx(col as u32, row as u32);
            cells[i] = Cell {
                glyph,
                shade: shade_u8(0.4),
            };
        }
    }
}
