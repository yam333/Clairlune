//! Crystal cave: the cave's chamber filled with giant crystals, clusters of
//! tall faceted columns rising from the floor, others hanging from the
//! ceiling, glints all around; no sky. The focal group stands on a rock
//! ledge.

use super::layers::put;
use super::{Ctx, cave};
use crate::glyphs::{EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    // The rock arch, the floor and the stalactites, as in a cave.
    let sky = cave::background(ctx, cells);
    let co = &ctx.co;
    let seed = hash_label(ctx.terrain, "crystals");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let g0 = co.col_of(f64::from(ctx.gx0)) - 4;
    let g1 = co.col_of(f64::from(ctx.gx1)) + 4;
    let clear = |c: i64| c < g0 || c > g1;
    let floor_row = layers_row(ctx, ctx.horizon + 0.02);

    // Clusters on the floor: a tall one in the middle, shorter ones leaning
    // out to the sides.
    let clusters = 3 + (pick(0, 1) * 3.0) as i64;
    for i in 0..clusters {
        let col = (pick(i, 2) * f64::from(ctx.cols)) as i64;
        if !clear(col - 6) || !clear(col + 6) {
            continue;
        }
        let tall = ctx.scaled(8.0 + 16.0 * pick(i, 3)).max(3);
        let base = floor_row + (pick(i, 4) * 3.0) as i64;
        let members = 3 + (pick(i, 5) * 3.0) as i64;
        for m in 0..members {
            let side = if m % 2 == 0 { 1 } else { -1 };
            let off = side * ((m + 1) / 2) * 2;
            let h = (tall as f64 * (1.0 - 0.22 * ((m + 1) / 2) as f64)).max(3.0) as i64;
            let lean = if m == 0 { 0 } else { side };
            crystal(
                ctx,
                cells,
                col + off,
                base,
                h,
                lean,
                false,
                hash3(seed, i, m),
            );
        }
    }
    // Crystals hanging from the ceiling: down from the lowest rock in a column.
    for i in 0..6 {
        let col = (pick(i, 20) * f64::from(ctx.cols)) as i64;
        if col < 1 || col >= i64::from(ctx.cols) - 1 || !clear(col) {
            continue;
        }
        let Some(root) = ceiling_row(ctx, cells, col as u32) else {
            continue;
        };
        let h = ctx.scaled(3.0 + 7.0 * pick(i, 21)).max(2);
        crystal(ctx, cells, col, root + 1, h, 0, true, hash3(seed, i, 22));
    }
    // Glints in the dark around them.
    let glints = hash_label(seed, "glints");
    for row in 0..ctx.platform_row {
        for col in 0..ctx.cols {
            if cells[ctx.idx(col, row)].glyph != EMPTY || !clear(i64::from(col)) {
                continue;
            }
            let k = hash3(glints, i64::from(col), i64::from(row));
            if unit_from_hash(k) < 0.006 {
                let glyph = [id('*'), id('+'), id('\''), id('.')][(k >> 30) as usize % 4];
                put(ctx, cells, i64::from(col), i64::from(row), glyph, 0.75);
            }
        }
    }
    sky
}

/// The first row at or below normalised y.
fn layers_row(ctx: &Ctx, y: f64) -> i64 {
    super::layers::row_at(ctx, y)
}

/// The lowest row of the rock hanging over a column, if any (the ceiling).
fn ceiling_row(ctx: &Ctx, cells: &[Cell], col: u32) -> Option<i64> {
    let mut last = None;
    for row in 0..ctx.platform_row {
        if cells[ctx.idx(col, row)].glyph == EMPTY {
            break;
        }
        last = Some(i64::from(row));
    }
    last.filter(|&r| r > 0)
}

/// One crystal: a pointed column `h` rows tall, a tip, facet edges and a
/// bright core, leaning a column every few rows. Hanging ones point down.
#[allow(clippy::too_many_arguments)]
fn crystal(
    ctx: &Ctx,
    cells: &mut [Cell],
    col: i64,
    base: i64,
    h: i64,
    lean: i64,
    hanging: bool,
    seed: u64,
) {
    let wide = h > 10;
    for i in 0..h {
        // `i` counts from the base (the floor, or the ceiling) to the tip.
        let row = if hanging { base + i } else { base - i };
        let shift = lean * (i / 4);
        let c = col + shift;
        let from_tip = h - 1 - i;
        let glow = 0.62 + 0.3 * (1.0 - i as f64 / h as f64).min(1.0);
        if from_tip == 0 {
            put(
                ctx,
                cells,
                c,
                row,
                if hanging { id('v') } else { id('^') },
                0.92,
            );
            continue;
        }
        if from_tip == 1 {
            let (l, r) = if hanging {
                (id('\\'), id('/'))
            } else {
                (id('/'), id('\\'))
            };
            put(ctx, cells, c - 1, row, l, 0.7);
            put(ctx, cells, c, row, id('!'), 0.9);
            put(ctx, cells, c + 1, row, r, 0.7);
            continue;
        }
        let k = hash3(seed, i, 3);
        put(ctx, cells, c - 1, row, id('|'), 0.6);
        put(
            ctx,
            cells,
            c,
            row,
            if k.is_multiple_of(5) {
                id(':')
            } else {
                id('!')
            },
            glow,
        );
        put(ctx, cells, c + 1, row, id('|'), 0.55);
        if wide && from_tip > 3 {
            put(ctx, cells, c - 2, row, id('/'), 0.45);
            put(ctx, cells, c + 2, row, id('\\'), 0.42);
        }
    }
}
