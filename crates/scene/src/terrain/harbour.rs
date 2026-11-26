//! Harbour: a quay across the water lined with warehouses and tall houses,
//! a crane at one end, boats moored along it with their masts up, lamps on
//! the quay; windows and lamps lit after dusk, and all of it mirrored in the
//! water. The focal group stands on a stone ledge on this side.

use super::layers::{self, Layer, put};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

/// Hills beyond the town.
const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.2,
    amp: 0.03,
    freq: 1.0,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.52,
    shade: 0.22,
    outline: Some(0.32),
    snow: None,
    label: "harbour_hills",
};

/// Where the quay's edge is, above the horizon.
const QUAY: f64 = 0.055;

const HULL: &[&str] = &["\\=====/", " \\___/ "];
const HULL_LONG: &[&str] = &["\\=========/", " \\_______/ "];
const CABIN: &[&str] = &["_", "#"];

/// Where the water starts, under the quay's edge (normalised y).
pub(crate) fn water_top(ctx: &Ctx) -> f64 {
    let row = layers::row_at(ctx, ctx.horizon - QUAY) + 1;
    ctx.co.ny(row.max(0) as u32) - 0.5 / f64::from(ctx.rows)
}

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::GoldenHour | TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "harbour");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let evening = lit(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[HILLS]);
    // Nothing of the hills below the quay.
    let quay_top = ctx.horizon - QUAY;
    let quay_row = layers::row_at(ctx, quay_top);
    for row in quay_row.max(0)..i64::from(ctx.rows) {
        for col in 0..ctx.cols {
            cells[ctx.idx(col, row as u32)] = Cell::default();
        }
    }

    // The buildings along the quay, edge to edge.
    let windows = hash_label(seed, "windows");
    let mut col = -(pick(0, 1) * 6.0) as i64;
    let mut i = 0;
    let crane_at = if ctx.group_cx < w * 0.5 { 1 } else { 0 };
    let mut crane_col = None;
    while col < i64::from(ctx.cols) {
        i += 1;
        let width = 9 + (pick(i, 2) * 10.0) as i64;
        let height = ctx.scaled(8.0 + 16.0 * pick(i, 3)).max(4);
        let base = quay_row - 1;
        let top = base - height;
        let gable = pick(i, 4) < 0.55;
        let (l, r) = (col, col + width - 1);
        for row in top..=base {
            put(ctx, cells, l, row, id('|'), 0.42);
            put(ctx, cells, r, row, id('|'), 0.42);
            for c in l + 1..r {
                // Windows in a grid, some lit after dark.
                let wrow = (base - row) % 3 == 1 && (c - l) % 3 == 2 && row > top;
                let glyph = if wrow {
                    let on = evening && unit_from_hash(hash3(windows, c, row)) < 0.55;
                    put(
                        ctx,
                        cells,
                        c,
                        row,
                        if on { id('o') } else { id('.') },
                        if on { 0.95 } else { 0.32 },
                    );
                    continue;
                } else if row == top {
                    id('_')
                } else {
                    EMPTY
                };
                if glyph != EMPTY {
                    put(ctx, cells, c, row, glyph, 0.4);
                }
            }
        }
        let mut roof_top = top;
        if gable {
            // A pitched roof, as wide as the house.
            let half = width / 2;
            for k in 1..=(half / 2).max(1) {
                let row = top - k;
                let (a, b) = (l + 2 * k - 1, r - 2 * k + 1);
                if a > b {
                    break;
                }
                put(ctx, cells, a, row, id('/'), 0.45);
                put(ctx, cells, b, row, id('\\'), 0.45);
                for c in a + 1..b {
                    put(ctx, cells, c, row, id('='), 0.3);
                }
                roof_top = row;
            }
        }
        let y = co.ny(roof_top.max(0) as u32) - 0.5 / rows;
        for c in l.max(0)..=r.min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
        if i == 2 + crane_at * 6 && crane_col.is_none() {
            crane_col = Some(r + 3);
            col = r + 10;
        } else {
            col = r + 1 + (pick(i, 5) * 3.0) as i64;
        }
    }

    // A dockside crane: a lattice tower and its jib over the water.
    if let Some(cc) = crane_col {
        let base = quay_row - 1;
        let tall = ctx.scaled(30.0).max(10);
        for k in 0..tall {
            let row = base - k;
            put(ctx, cells, cc - 1, row, id('|'), 0.45);
            put(ctx, cells, cc + 1, row, id('|'), 0.45);
            put(
                ctx,
                cells,
                cc,
                row,
                if k % 2 == 0 { id('X') } else { id('+') },
                0.35,
            );
        }
        let jib = base - tall;
        let toward = if ctx.group_cx < w * 0.5 { -1 } else { 1 };
        for k in -6..=18 {
            put(ctx, cells, cc + toward * k, jib, id('='), 0.5);
        }
        let hook = cc + toward * 14;
        for k in 1..ctx.scaled(10.0).max(3) {
            put(ctx, cells, hook, jib + k, id('|'), 0.35);
        }
        put(
            ctx,
            cells,
            hook,
            jib + ctx.scaled(10.0).max(3),
            id('J'),
            0.45,
        );
        let y = co.ny((jib - 1).max(0) as u32);
        for c in (cc - 7).max(0)..=(cc + 19).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, c, y);
        }
    }

    // The quay's edge, and its lamps.
    for c in 0..i64::from(ctx.cols) {
        put(ctx, cells, c, quay_row, id('='), 0.5);
    }
    let lamps = 4 + (pick(0, 6) * 3.0) as i64;
    for k in 0..lamps {
        let c = (f64::from(ctx.cols) * (k as f64 + 0.3 + 0.4 * pick(k, 7)) / lamps as f64) as i64;
        let post = ctx.scaled(5.0).max(3);
        for j in 1..post {
            put(ctx, cells, c, quay_row - j, id('|'), 0.45);
        }
        put(
            ctx,
            cells,
            c,
            quay_row - post,
            id('o'),
            if evening { 1.0 } else { 0.5 },
        );
    }

    // Boats moored along the quay, masts up.
    let boats = 3 + (pick(0, 8) * 3.0) as i64;
    for b in 0..boats {
        let c = (f64::from(ctx.cols) * (b as f64 + 0.2 + 0.6 * pick(b, 9)) / boats as f64) as i64;
        if (co.nx(c.clamp(0, i64::from(ctx.cols) - 1) as u32) * w - ctx.group_cx).abs() < w * 0.06 {
            continue;
        }
        let long = pick(b, 10) < 0.4;
        let hull = if long { HULL_LONG } else { HULL };
        let row = quay_row + 2;
        layers::stamp(ctx, cells, hull, c, row, 0.5);
        let mast = ctx
            .scaled(if long { 22.0 } else { 14.0 } * (0.8 + 0.4 * pick(b, 11)))
            .max(5);
        let deck = row - hull.len() as i64;
        for k in 0..mast {
            put(ctx, cells, c, deck - k, id('|'), 0.45);
        }
        // Rigging, from the masthead to bow and stern.
        let reach = hull[0].len() as i64 / 2;
        for k in 1..mast {
            let x = reach * k / mast;
            if k % 2 == 0 {
                put(ctx, cells, c - x, deck - mast + k, id('/'), 0.3);
                put(ctx, cells, c + x, deck - mast + k, id('\\'), 0.3);
            }
        }
        layers::stamp(ctx, cells, CABIN, c + if long { 3 } else { 2 }, deck, 0.5);
        if long {
            put(
                ctx,
                cells,
                c - 2,
                deck,
                id('o'),
                if evening { 0.9 } else { 0.45 },
            );
        }
    }

    // The water: what is above, mirrored, rippled, fading.
    let ripple_seed = hash_label(seed, "ripples");
    let above: Vec<Cell> = cells.to_vec();
    let first = (quay_row + 1).max(0);
    for row in first..i64::from(ctx.rows) {
        let depth = row - first;
        let fade = 1.0 - (depth as f64 / (rows - first as f64).max(1.0)).min(1.0);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row as u32);
            // The boats float on it.
            if row <= quay_row + 2 && cells[idx].glyph != EMPTY {
                continue;
            }
            let n = fbm2(ripple_seed, co.nx(col) * 40.0, row as f64 * 0.7, 2);
            let shift = if n > 0.62 { 1 } else { 0 };
            let src_row = quay_row - 1 - depth;
            let src_col = (i64::from(col) + shift).min(i64::from(ctx.cols) - 1);
            let mirrored = if src_row >= 0 {
                above[ctx.idx(src_col as u32, src_row as u32)]
            } else {
                Cell::default()
            };
            cells[idx] = if mirrored.glyph != EMPTY && n < 0.7 {
                // Lit things stay bright in the water.
                let bright = f64::from(mirrored.shade) / 255.0 > 0.85;
                Cell {
                    glyph: if bright { id(':') } else { mirrored.glyph },
                    shade: shade_u8(f64::from(mirrored.shade) / 255.0 * (0.35 + 0.35 * fade)),
                }
            } else if n > 0.66 {
                Cell {
                    glyph: id('~'),
                    shade: shade_u8(0.25 + 0.1 * (1.0 - fade)),
                }
            } else {
                Cell::default()
            };
        }
    }
    sky
}
