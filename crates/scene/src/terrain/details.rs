//! Extra details for the sparser landscapes, from generator 4 on (older
//! recipes keep their scenes exactly):
//!
//! - desert: a big sun (striped, low on the horizon) or a rising moon behind
//!   the mesas, a tall saguaro in the foreground, cacti and brush on the dunes;
//! - shore: a moon after dark with its path of light on the water (the sun's
//!   glare by day), a rocky headland with a lighthouse, boats, gulls;
//! - night sky: constellations and a shooting star.
//!
//! All placed from the seed, clear of the focal group.

use super::layers::{Layer, put, row_at, stamp};
use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::{fbm1, fbm2};
use crate::recipe::TimeOfDay;

/// Whether this recipe gets the details.
pub(crate) fn enabled(ctx: &Ctx) -> bool {
    ctx.recipe.generator_version >= 4
}

fn pick(seed: u64, i: i64, salt: i64) -> f64 {
    unit_from_hash(hash3(seed, i, salt))
}

fn group_left(ctx: &Ctx) -> bool {
    ctx.group_cx < ctx.co.w * 0.5
}

/// Whether a stamp `half` columns either side of `col` stays clear of the
/// focal group (with a few columns to spare).
fn clear_of_group(ctx: &Ctx, col: i64, half: i64) -> bool {
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - 3;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + 3;
    col + half < g0 || col - half > g1
}

/// Normalised y of a row's top edge.
fn row_top(ctx: &Ctx, row: i64) -> f64 {
    (f64::from(ctx.co.g.origin_y) + row as f64 * f64::from(ctx.co.g.cell_h)) / ctx.co.h
}

/// Keeps the sky pass's stars off columns `c0..=c1` from `top_row` down.
fn shelter(ctx: &Ctx, sky: &mut [f64], c0: i64, c1: i64, top_row: i64) {
    let y = row_top(ctx, top_row - 1);
    let mut before = ctx.unsheltered.borrow_mut();
    for c in c0.max(0)..=c1.min(i64::from(ctx.cols) - 1) {
        let s = &mut sky[c as usize];
        before[c as usize].get_or_insert(*s);
        *s = s.min(y);
    }
}

fn width(shape: &[&str]) -> i64 {
    shape.iter().map(|l| l.len()).max().unwrap_or(0) as i64
}

/// Writes one cell if it is on screen, at full shade even below the platform
/// line (`put` dims there): for the few things meant to catch the eye.
fn put_bright(ctx: &Ctx, cells: &mut [Cell], col: i64, row: i64, glyph: u8, shade: f64) {
    if col < 0 || row < 0 || col >= i64::from(ctx.cols) || row >= i64::from(ctx.rows) {
        return;
    }
    cells[ctx.idx(col as u32, row as u32)] = Cell {
        glyph,
        shade: shade_u8(shade),
    };
}

/// Like `stamp` (bottom-centre on `(col, row)`), but solid: the spaces inside
/// each line's outline are cleared, and nothing is dimmed.
fn stamp_solid(ctx: &Ctx, cells: &mut [Cell], shape: &[&str], col: i64, row: i64, shade: f64) {
    let h = shape.len() as i64;
    for (dy, line) in shape.iter().enumerate() {
        let w = line.len() as i64;
        let (Some(first), Some(last)) = (line.find(|c| c != ' '), line.rfind(|c| c != ' ')) else {
            continue;
        };
        let r = row - h + 1 + dy as i64;
        for (dx, ch) in line.chars().enumerate().take(last + 1).skip(first) {
            let c = col - w / 2 + dx as i64;
            let (glyph, s) = if ch == ' ' {
                (EMPTY, 0.0)
            } else {
                (id(ch), shade)
            };
            put_bright(ctx, cells, c, r, glyph, s);
        }
    }
}

/// The disc's bounds in cells, and `f(col, row, u, v)` for each cell whose
/// centre is inside it (`u`, `v` in -1..1, `v` down).
fn disc(
    ctx: &Ctx,
    cx: f64,
    cy: f64,
    r: f64,
    mut f: impl FnMut(i64, i64, f64, f64),
) -> (i64, i64, i64) {
    let g = &ctx.co.g;
    let (cw, ch) = (f64::from(g.cell_w), f64::from(g.cell_h));
    let (ox, oy) = (f64::from(g.origin_x), f64::from(g.origin_y));
    let c_lo = ((cx - r - ox) / cw).floor() as i64;
    let c_hi = ((cx + r - ox) / cw).ceil() as i64;
    let r_lo = ((cy - r - oy) / ch).floor() as i64;
    let r_hi = ((cy + r - oy) / ch).ceil() as i64;
    for row in r_lo..=r_hi {
        for col in c_lo..=c_hi {
            let u = (ox + (col as f64 + 0.5) * cw - cx) / r;
            let v = (oy + (row as f64 + 0.5) * ch - cy) / r;
            if u * u + v * v <= 1.0 {
                f(col, row, u, v);
            }
        }
    }
    (c_lo, c_hi, r_lo)
}

// ---- Desert -------------------------------------------------------------------

/// Where the desert's sun or moon sits, if anywhere: its columns and top row.
pub(crate) struct Disc {
    c0: i64,
    c1: i64,
    top: i64,
}

/// A big sun low on the horizon, striped below its middle, at sunrise, golden
/// hour and dusk; a smaller one high up at noon; a big pale moon rising after
/// dark. Painted before the mesas, which then cut it.
pub(crate) fn desert_sky(ctx: &Ctx, cells: &mut [Cell]) -> Option<Disc> {
    let seed = hash_label(ctx.terrain, "desert_sky");
    let side = 0.66 + 0.16 * pick(seed, 1, 0);
    let cx = ctx.co.w * if group_left(ctx) { side } else { 1.0 - side };
    let r = ctx.unit * (0.14 + 0.05 * pick(seed, 2, 0));
    let cy = ctx.co.h * (ctx.horizon - 0.09);
    let centre = row_at(ctx, cy / ctx.co.h);
    // Nothing below the ground line: the layout there is the same in both
    // themes, and the dunes cover it anyway.
    let ground = i64::from(ctx.platform_row);
    let (c0, c1, top) = match ctx.tod {
        TimeOfDay::Noon => {
            let cy = ctx.co.h * (0.12 + 0.1 * pick(seed, 3, 0));
            disc(ctx, cx, cy, r * 0.45, |c, row, u, v| {
                let rim = u * u + v * v > 0.7;
                let glyph = if rim { id('-') } else { id('=') };
                put(ctx, cells, c, row, glyph, 0.36);
            })
        }
        TimeOfDay::Morning | TimeOfDay::GoldenHour | TimeOfDay::Dusk => {
            disc(ctx, cx, cy, r, |c, row, u, v| {
                if row >= ground {
                    return;
                }
                // Gaps between the stripes widen toward the bottom.
                let below = row - centre;
                let gap = match below {
                    ..=0 => false,
                    b if v < 0.35 => b % 4 == 0,
                    b if v < 0.7 => b % 3 == 0,
                    b => b % 2 == 0,
                };
                if gap {
                    put(ctx, cells, c, row, EMPTY, 0.0);
                    return;
                }
                let rim = u * u + v * v > 0.8;
                let glyph = if rim { id('-') } else { id('=') };
                put(ctx, cells, c, row, glyph, 0.42 - 0.1 * v.max(0.0));
            })
        }
        TimeOfDay::DeepDusk | TimeOfDay::Night => {
            const FACE: [u8; 4] = [id('.'), id('.'), id(':'), id('o')];
            disc(ctx, cx, cy, r, |c, row, u, v| {
                if row >= ground {
                    return;
                }
                let d2 = u * u + v * v;
                if d2 > 0.86 {
                    put(ctx, cells, c, row, id('.'), 0.45);
                    return;
                }
                let n = fbm2(seed, u * 2.4 + 3.0, v * 2.4, 3);
                let d = clamp01((n - 0.5) * 2.4 + 0.3);
                let glyph = family_glyph(&FACE, d, 0.0, seed, c.max(0) as u32, row.max(0) as u32);
                put(ctx, cells, c, row, glyph, 0.22 + 0.16 * d);
            })
        }
    };
    Some(Disc { c0, c1, top })
}

const BRUSH: [&str; 2] = ["\\|/", " | "];
const SAGUARO: [&str; 3] = ["  |  ", "|_|_|", "  |  "];
const SAGUARO_TALL: [&str; 4] = ["  |  ", "| | |", "'-|-'", "  |  "];
const SAGUARO_NEAR: [&str; 11] = [
    "    _    ",
    "   | | _ ",
    " _ | || |",
    "| || || |",
    "| || |_/ ",
    "| || |   ",
    " \\_| |   ",
    "   | |   ",
    "   | |   ",
    "   | |   ",
    "   | |   ",
];

/// Cacti and brush along the dune crests (a few on the far dunes where the
/// near ones don't hide them, more on the near dunes), a tall saguaro at the
/// screen edge on the group's side, and the sun or moon kept free of stars.
pub(crate) fn desert_plants(
    ctx: &Ctx,
    cells: &mut [Cell],
    sky: &mut [f64],
    far: &Layer,
    near: &Layer,
    sun: Option<Disc>,
) {
    if let Some(s) = sun {
        shelter(ctx, sky, s.c0, s.c1, s.top);
    }
    let seed = hash_label(ctx.terrain, "desert_plants");
    for (li, layer, spacing, shade) in [(0i64, far, 28u32, 0.55), (1, near, 18, 0.8)] {
        let mut col = 0u32;
        let mut i = 0i64;
        while col < ctx.cols {
            i += 1;
            let k = pick(seed, i, li);
            let c = col + (pick(seed, i, 10 + li) * f64::from(spacing)) as u32;
            col += spacing;
            if k < 0.45 || c >= ctx.cols {
                continue;
            }
            let shape: &[&str] = match ((k - 0.45) / 0.55 * 3.0) as u32 {
                0 => &BRUSH,
                1 => &SAGUARO,
                _ => &SAGUARO_TALL,
            };
            let half = width(shape) / 2;
            let nx = ctx.co.nx(c);
            let hidden = li == 0 && near.top(ctx, nx) < far.top(ctx, nx) + 0.03;
            if hidden || !clear_of_group(ctx, i64::from(c), half) {
                continue;
            }
            let row = row_at(ctx, layer.top(ctx, nx));
            stamp(ctx, cells, shape, i64::from(c), row, shade);
            let c = i64::from(c);
            shelter(ctx, sky, c - half, c + half, row - shape.len() as i64 + 1);
        }
    }
    // The tall one, in front, at the edge nearer the group.
    let half = width(&SAGUARO_NEAR) / 2;
    let edge = 0.03 + 0.05 * pick(seed, 0, 99);
    let nx = if group_left(ctx) { edge } else { 1.0 - edge };
    let col = ctx.co.col_of(nx * ctx.co.w);
    if pick(seed, 1, 99) < 0.75 && clear_of_group(ctx, col, half + 2) {
        let row = row_at(ctx, near.top(ctx, nx)) + 1;
        stamp_solid(ctx, cells, &SAGUARO_NEAR, col, row, 0.62);
        shelter(
            ctx,
            sky,
            col - half,
            col + half,
            row - SAGUARO_NEAR.len() as i64 + 1,
        );
    }
}

// ---- Shore --------------------------------------------------------------------

const SAIL: [&str; 3] = [" |\\ ", " |_\\", "\\___/"];
const SAIL_FAR: [&str; 2] = ["|\\", "--"];
const LIGHTHOUSE: [&str; 5] = [" _ ", "|O|", "|=|", "| |", "|_|"];

/// The moon (after dark) and its path of light on the water, or the sun's
/// glare by day; a rocky headland at the screen edge away from the pier,
/// sometimes with a lighthouse; boats on the sea line; gulls by day.
pub(crate) fn shore_life(ctx: &Ctx, cells: &mut [Cell], sky: &mut [f64], sea_top: f64) {
    let seed = hash_label(ctx.terrain, "shore_life");
    // The pier comes in from the edge nearer the group (see `pier`).
    let pier_left = group_left(ctx);
    let from_edge = |nx: f64| if pier_left { 1.0 - nx } else { nx };
    let sea_row = row_at(ctx, sea_top);
    let reach = 0.10 + 0.07 * pick(seed, 1, 0);

    // The moon or sun over open water, between the group and the headland.
    let d = reach + 0.08 + (0.32 - reach) * pick(seed, 6, 0);
    let mx = if pier_left { 1.0 - d } else { d };
    let dark = !TimeOfDay::LIGHT.contains(&ctx.tod);
    let mc = ctx.co.col_of(mx * ctx.co.w);
    // The path's size doesn't depend on the time of day, so the layout below
    // the horizon stays the same in both themes.
    let r = ctx.unit * (0.035 + 0.02 * pick(seed, 7, 0));
    let path_half = r / f64::from(ctx.co.g.cell_w);
    if dark {
        const FACE: [u8; 5] = [id(':'), id('o'), id('O'), id('%'), id('@')];
        let cy = ctx.co.h * (0.14 + 0.16 * pick(seed, 8, 0));
        let (c0, c1, top) = disc(ctx, mx * ctx.co.w, cy, r, |c, row, u, v| {
            let n = fbm2(seed ^ 0xC4A7, u * 2.2 + 5.0, v * 2.2, 3);
            let d = clamp01(0.95 - 0.5 * clamp01((n - 0.55) * 7.0) - 0.15 * (u * u + v * v));
            let glyph = family_glyph(&FACE, d, 0.0, seed, c.max(0) as u32, row.max(0) as u32);
            put(ctx, cells, c, row, glyph, 0.55 + 0.35 * d);
        });
        shelter(ctx, sky, c0 - 1, c1 + 1, top);
    }
    // The path: broken bright marks under the light, widening toward the
    // viewer, never under the pier.
    let span = (i64::from(ctx.rows) - sea_row).max(1) as f64;
    for row in sea_row + 1..i64::from(ctx.rows) {
        let t = (row - sea_row) as f64 / span;
        let half = path_half * (0.5 + 2.5 * t);
        let wobble = (fbm1(seed ^ 0x9A7, row as f64 * 0.3, 2) - 0.5) * 4.0;
        let lo = (mc as f64 - half + wobble).round() as i64;
        let hi = (mc as f64 + half + wobble).round() as i64;
        for c in lo..=hi {
            if !clear_of_group(ctx, c, 0)
                || (row >= i64::from(ctx.platform_row) && !clear_of_group(ctx, c, 8))
            {
                continue;
            }
            let edge = ((c as f64 - mc as f64 - wobble) / half.max(1.0)).abs();
            let k = pick(seed, c * 131 + row, 9);
            if k < 0.12 + 0.75 * edge * edge {
                continue;
            }
            let glyph = if k > 0.85 {
                id('=')
            } else if k > 0.6 {
                id('-')
            } else {
                id('~')
            };
            let shade = if dark { 0.72 } else { 0.55 } - 0.25 * edge - 0.2 * t;
            put_bright(ctx, cells, c, row, glyph, shade);
        }
    }

    // Headland: rises steeply from the sea toward the edge, rough on top.
    let height = 0.07 + 0.06 * pick(seed, 2, 0);
    let mut peak: Option<(i64, i64)> = None;
    for col in 0..ctx.cols {
        let nx = ctx.co.nx(col);
        let d = from_edge(nx);
        if d > reach || !clear_of_group(ctx, i64::from(col), 0) {
            continue;
        }
        let rise = crate::math::pow(1.0 - d / reach, 0.5);
        let rough = 0.025 * (fbm1(seed, nx * 20.0, 3) - 0.5);
        let top = sea_top - height * rise + rough;
        let r0 = row_at(ctx, top).min(sea_row);
        for row in r0..=sea_row {
            let depth = (row - r0) as f64 / (sea_row - r0).max(1) as f64;
            let glyph = if row == r0 {
                if d < reach * 0.15 { id('/') } else { id('_') }
            } else {
                let n = fbm2(seed ^ 0x40C, nx * ctx.aspect * 40.0, row as f64 * 0.45, 3);
                let density = clamp01((n - 0.5) * 2.0 + 0.35 + 0.35 * depth);
                if density < 0.25 {
                    EMPTY
                } else {
                    family_glyph(&glyphs::RIDGE, density, 0.25, seed, col, row as u32)
                }
            };
            put(ctx, cells, i64::from(col), row, glyph, 0.32 + 0.18 * depth);
        }
        shelter(ctx, sky, i64::from(col), i64::from(col), r0);
        if d > reach * 0.3 && d < reach * 0.8 && peak.is_none_or(|(_, r)| r0 < r) {
            peak = Some((i64::from(col), r0));
        }
    }
    if let Some((c, r)) = peak
        && pick(seed, 3, 0) < 0.6
    {
        stamp(ctx, cells, &LIGHTHOUSE, c, r - 1, 0.65);
        shelter(ctx, sky, c - 1, c + 1, r - LIGHTHOUSE.len() as i64);
    }

    // Boats on the sea line, clear of the group, the headland and each other.
    let boats = 1 + (pick(seed, 4, 0) * 3.0) as i64;
    let mut taken: Vec<i64> = Vec::new();
    for b in 0..boats {
        let nx = 0.06 + 0.88 * pick(seed, 10 + b, 1);
        let col = ctx.co.col_of(nx * ctx.co.w);
        let (shape, shade): (&[&str], f64) = if pick(seed, 20 + b, 1) < 0.5 {
            (&SAIL, 0.6)
        } else {
            (&SAIL_FAR, 0.45)
        };
        let half = width(shape) / 2;
        if from_edge(nx) < reach + 0.04
            || !clear_of_group(ctx, col, half + 4)
            || taken.iter().any(|&t| (t - col).abs() < 10)
        {
            continue;
        }
        taken.push(col);
        stamp(ctx, cells, shape, col, sea_row, shade);
        shelter(
            ctx,
            sky,
            col - half,
            col + half,
            sea_row - shape.len() as i64 + 1,
        );
    }

    // Gulls in daylight, high up.
    if !dark {
        let gulls = 2 + (pick(seed, 5, 0) * 4.0) as i64;
        for g in 0..gulls {
            let col = ctx
                .co
                .col_of((0.08 + 0.84 * pick(seed, 30 + g, 2)) * ctx.co.w);
            let row = row_at(ctx, 0.08 + 0.3 * pick(seed, 40 + g, 2));
            if !clear_of_group(ctx, col, 2) {
                continue;
            }
            if pick(seed, 50 + g, 2) < 0.5 {
                put(ctx, cells, col, row, id('v'), 0.55);
            } else {
                put(ctx, cells, col, row, id('\\'), 0.55);
                put(ctx, cells, col + 1, row, id('/'), 0.55);
            }
        }
    }
}

// ---- Night sky ----------------------------------------------------------------

/// After dusk, one or two constellations (bright stars joined by faint dotted
/// lines) on the side away from the moon; at night, a shooting star.
pub(crate) fn sky_patterns(ctx: &Ctx, cells: &mut [Cell]) {
    if !TimeOfDay::DARK.contains(&ctx.tod) {
        return;
    }
    let seed = hash_label(ctx.terrain, "sky_patterns");
    // The moon sits on the side away from the group (see `night_sky::moon`),
    // so the constellations go on the group's side, high above it.
    let count = 1 + i64::from(pick(seed, 1, 0) < 0.6);
    for k in 0..count {
        let lo = if group_left(ctx) { 0.04 } else { 0.54 } + 0.2 * k as f64;
        let mut x = lo + 0.03 + 0.08 * pick(seed, 2, k);
        let mut y = 0.06 + 0.12 * pick(seed, 3, k);
        let n = 4 + (pick(seed, 4, k) * 3.0) as i64;
        let mut pts: Vec<(i64, i64)> = Vec::new();
        for i in 0..n {
            let col = ctx.co.col_of(x * ctx.co.w);
            let row = row_at(ctx, y);
            if row >= 1
                && row < i64::from(ctx.platform_row)
                && (0..i64::from(ctx.cols)).contains(&col)
            {
                pts.push((col, row));
            }
            x = (x + 0.025 + 0.04 * pick(seed, 5 + i, k)).min(lo + 0.4);
            y = (y + (pick(seed, 15 + i, k) - 0.5) * 0.1).clamp(0.03, 0.32);
        }
        for w in pts.windows(2) {
            let ((c0, r0), (c1, r1)) = (w[0], w[1]);
            let steps = (c1 - c0).abs().max((r1 - r0).abs() * 2).max(1);
            for s in 1..steps {
                let (c, r) = (c0 + (c1 - c0) * s / steps, r0 + (r1 - r0) * s / steps);
                if s % 2 == 0 && empty(ctx, cells, c, r) {
                    put(ctx, cells, c, r, id('.'), 0.32);
                }
            }
        }
        for &(c, r) in &pts {
            put(ctx, cells, c, r, id('*'), 0.85);
        }
    }
    if matches!(ctx.tod, TimeOfDay::Night | TimeOfDay::DeepDusk) {
        // A shooting star: a bright head and a fading streak behind it.
        let col = ctx.co.col_of((0.4 + 0.2 * pick(seed, 60, 0)) * ctx.co.w);
        let row = row_at(ctx, 0.08 + 0.08 * pick(seed, 61, 0));
        let dir = if pick(seed, 62, 0) < 0.5 { 1 } else { -1 };
        let trail = if dir > 0 { id('\\') } else { id('/') };
        put(ctx, cells, col, row, id('*'), 0.9);
        for i in 1..=7i64 {
            put(
                ctx,
                cells,
                col - dir * i,
                row - i,
                trail,
                0.7 - 0.08 * i as f64,
            );
        }
    }
}

fn empty(ctx: &Ctx, cells: &[Cell], col: i64, row: i64) -> bool {
    col >= 0
        && row >= 0
        && col < i64::from(ctx.cols)
        && row < i64::from(ctx.rows)
        && cells[ctx.idx(col as u32, row as u32)].glyph == EMPTY
}
