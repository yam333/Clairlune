//! Lantern lake: a still lake at the foot of wooded hills, a pagoda on the
//! far shore, a gate standing in the water, paper lanterns floating on it
//! and more rising into the sky, all lit after dark; the focal group stands
//! on a grassy bank.

use super::layers::{self, Layer, put};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

const HILLS: Layer = Layer {
    family: &glyphs::FOLIAGE,
    hatch: None,
    base: -0.085,
    amp: 0.045,
    freq: 1.8,
    ridged: false,
    plateau: None,
    fill: (30.0, 30.0),
    threshold: 0.42,
    shade: 0.32,
    outline: Some(0.42),
    snow: None,
    label: "lantern_hills",
};

/// Where the water begins, below the horizon.
const WATER_LINE: f64 = -0.035;

const PAGODA: [&str; 10] = [
    "    ^    ",
    "   /=\\   ",
    "  /___\\  ",
    "   |#|   ",
    " /=====\\ ",
    "/_______\\",
    "   |#|   ",
    " /=====\\ ",
    "/_______\\",
    "  |   |  ",
];

const GATE: [&str; 7] = [
    "_=========_",
    "  |=====|  ",
    "  |     |  ",
    "  |     |  ",
    "  |     |  ",
    "  |     |  ",
    "  |     |  ",
];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "lantern_lake");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x1A7));
    let lit = matches!(
        ctx.tod,
        TimeOfDay::GoldenHour | TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    );
    let lantern = if lit { 0.95 } else { 0.55 };
    let mut sky = layers::paint(ctx, cells, &[HILLS]);
    let group_left = ctx.group_cx < w * 0.5;
    let shelter = |sky: &mut [f64], col: i64, half: i64, top_row: i64| {
        let y = co.ny(top_row.max(0) as u32) - 0.5 / rows;
        for c in (col - half).max(0)..=(col + half).min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
    };

    // A pagoda on the far shore, on the side away from the group.
    let px = if group_left {
        0.78 + 0.12 * pick(1)
    } else {
        0.1 + 0.12 * pick(1)
    };
    let pcol = co.col_of(px * w);
    if pcol > 5 && pcol < i64::from(ctx.cols) - 6 {
        let row = layers::row_at(ctx, HILLS.top(ctx, px)) + 2;
        layers::stamp(ctx, cells, &PAGODA, pcol, row, 0.4);
        // Its windows lit.
        put(
            ctx,
            cells,
            pcol,
            row - 6,
            id('#'),
            if lit { 0.9 } else { 0.45 },
        );
        put(
            ctx,
            cells,
            pcol,
            row - 3,
            id('#'),
            if lit { 0.9 } else { 0.45 },
        );
        shelter(&mut sky, pcol, 4, row - PAGODA.len() as i64);
    }

    // The water: a mirror of the hills, the pagoda and the gate.
    let water_top = ctx.horizon + WATER_LINE;
    let first = (0..ctx.rows)
        .find(|&r| co.ny(r) >= water_top)
        .unwrap_or(ctx.rows);
    // The gate stands in the water, between the pagoda and the group.
    // Halfway between the group and the pagoda.
    let gx = (ctx.group_cx / w + px) * 0.5 + 0.04 * (pick(2) - 0.5);
    let gcol = co.col_of(gx * w);
    let gate_shown =
        (gx * w - ctx.group_cx).abs() > w * 0.14 && gcol > 6 && gcol < i64::from(ctx.cols) - 7;
    if gate_shown {
        let row = i64::from(first) + 1;
        layers::stamp(ctx, cells, &GATE, gcol, row, 0.5);
        shelter(&mut sky, gcol, 5, row - GATE.len() as i64);
    }
    let above: Vec<Cell> = cells.to_vec();
    let water = hash_label(seed, "water");
    for row in first..ctx.rows {
        let depth = row - first;
        let mirror = first.checked_sub(depth + 1);
        let fade = 1.0 - (f64::from(depth) / f64::from((ctx.rows - first).max(1))).min(1.0);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            if row == first {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.4),
                };
                continue;
            }
            // The gate's posts stand in the water as they are.
            if gate_shown
                && cells[idx].glyph != EMPTY
                && (i64::from(col) - gcol).abs() <= 5
                && depth <= 2
            {
                continue;
            }
            let ripple = fbm2(water, co.nx(col) * 36.0, f64::from(row) * 0.6, 2);
            let k = hash3(water, i64::from(col), i64::from(row));
            let src = mirror.map(|m| above[ctx.idx(col, m)]);
            cells[idx] = match src {
                Some(c) if c.glyph != EMPTY && ripple < 0.6 && unit_from_hash(k) < 0.8 => Cell {
                    glyph: if ripple > 0.55 { id('~') } else { c.glyph },
                    shade: shade_u8(f64::from(c.shade) / 255.0 * (0.4 + 0.15 * fade)),
                },
                _ if unit_from_hash(k.rotate_left(13)) < 0.04 => Cell {
                    glyph: id('-'),
                    shade: shade_u8(0.26),
                },
                _ => Cell::default(),
            };
        }
    }
    // The land's top, before the water's line: sky lanterns stay above it.
    let hills_top = sky.clone();
    for s in &mut sky {
        *s = s.min(water_top);
    }

    // Lanterns floating on the water, each with its light under it; more
    // of them nearer, none in front of the group.
    let floats = hash_label(seed, "floats");
    let g0 = co.col_of(f64::from(ctx.gx0)) - 3;
    let g1 = co.col_of(f64::from(ctx.gx1)) + 3;
    for row in first + 1..ctx.rows.saturating_sub(1) {
        let near = f64::from(row - first) / f64::from((ctx.rows - first).max(1));
        for col in 1..ctx.cols - 1 {
            let c = i64::from(col);
            if c >= g0 && c <= g1 {
                continue;
            }
            let k = hash3(floats, c, i64::from(row));
            if unit_from_hash(k) < 0.006 + 0.012 * near {
                put(ctx, cells, c, i64::from(row), id('o'), lantern);
                put(ctx, cells, c, i64::from(row) + 1, id('!'), lantern * 0.55);
            }
        }
    }

    // Sky lanterns rising, small and spread out, fewer higher up.
    let rising = hash_label(seed, "rising");
    for row in 0..first.saturating_sub(4) {
        let ny = co.ny(row);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            if cells[idx].glyph != EMPTY || ny > hills_top[col as usize] - 0.03 {
                continue;
            }
            let k = hash3(rising, i64::from(col), i64::from(row));
            let chance = 0.0035 * (ny / water_top).clamp(0.2, 1.0);
            if unit_from_hash(k) < chance {
                let glyph = if k & 3 == 0 { id('*') } else { id('o') };
                put(
                    ctx,
                    cells,
                    i64::from(col),
                    i64::from(row),
                    glyph,
                    lantern * 0.85,
                );
            }
        }
    }
    sky
}
