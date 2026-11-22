//! Wizard's tower: a crooked stone tower on a crag across the valley, its
//! pointed roof wrapped in a spiral of sparks, windows lit after dark,
//! rocks floating in the air about it; the focal group stands on a rock
//! ledge.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{TAU, clamp01, sin};
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

const PEAKS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: Some((&glyphs::FAR_LEFT, &glyphs::FAR_RIGHT)),
    base: -0.14,
    amp: 0.09,
    freq: 1.4,
    ridged: true,
    plateau: None,
    fill: (26.0, 14.0),
    threshold: 0.5,
    shade: 0.26,
    outline: Some(0.42),
    snow: None,
    label: "tower_peaks",
};

const SLOPES: Layer = Layer {
    family: &glyphs::RIDGE,
    hatch: None,
    base: 0.01,
    amp: 0.03,
    freq: 2.0,
    ridged: true,
    plateau: None,
    fill: (16.0, 26.0),
    threshold: 0.48,
    shade: 0.45,
    outline: Some(0.55),
    snow: None,
    label: "tower_slopes",
};

/// The crag's rock, light to heavy.
const ROCK: [u8; 4] = [id('.'), id(':'), id('x'), id('%')];

const ISLET: &[&str] = &["_%@%_", " \\%/ ", "  v  "];
const ISLET_SMALL: &[&str] = &["_@_", " v "];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "wizard_tower");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x70E));
    let lit = matches!(
        ctx.tod,
        TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    ) || matches!(ctx.tod, TimeOfDay::GoldenHour);
    let mut sky = layers::paint(ctx, cells, &[PEAKS]);

    // The crag: a steep knob of rock on the far side.
    let group_left = ctx.group_cx < w * 0.5;
    let cx = if group_left {
        w * (0.68 + 0.14 * pick(1))
    } else {
        w * (0.18 + 0.14 * pick(1))
    };
    let crag_w = w * (0.09 + 0.04 * pick(2));
    let crag_top = ctx.horizon - (0.14 + 0.05 * pick(3));
    let crag_seed = hash_label(seed, "crag");
    let top_of = |px: f64| -> Option<f64> {
        let u = (px - cx) / crag_w;
        if u.abs() >= 1.0 {
            return None;
        }
        // Steep sides, a rounded top.
        let p = (1.0 - u * u).powf(0.6);
        Some(ctx.horizon + 0.02 - (ctx.horizon + 0.02 - crag_top) * p)
    };
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let Some(t) = top_of(px) else {
            continue;
        };
        sky[col as usize] = sky[col as usize].min(t);
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < t {
                continue;
            }
            if ny > ctx.horizon + 0.05 {
                break;
            }
            let idx = ctx.idx(col, row);
            if (ny - t) * rows < 1.0 {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: super::shade_u8(0.55),
                };
                continue;
            }
            let n = fbm2(crag_seed, px / h * 14.0, ny * 20.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.5);
            cells[idx] = if d < 0.38 {
                Cell::default()
            } else {
                Cell {
                    glyph: family_glyph(&ROCK, d, 0.38, crag_seed, col, row),
                    shade: super::shade_u8(0.42),
                }
            };
        }
    }

    // The tower on the crag's top, leaning as it rises.
    let tcol = co.col_of(cx);
    let base = layers::row_at(ctx, crag_top) + 1;
    let height = ctx.scaled(24.0 + 12.0 * pick(4)).max(8);
    let lean = if pick(5) < 0.5 { -1 } else { 1 };
    let half = 3i64;
    let windows = hash_label(seed, "windows");
    let mut top_col = tcol;
    for i in 0..height {
        let row = base - i;
        let c = tcol + lean * ((i * i) / (height * height / 3).max(1));
        top_col = c;
        put(ctx, cells, c - half, row, id('|'), 0.55);
        put(ctx, cells, c + half, row, id('|'), 0.5);
        for x in c - half + 1..c + half {
            let glyph = if i % 4 == 0 { id('=') } else { id('#') };
            put(ctx, cells, x, row, glyph, 0.42);
        }
        // Windows here and there, one a floor.
        if i > 2 && i % 5 == 2 {
            let k = hash3(windows, i, 1);
            let wx = c - 1 + (k % 3) as i64;
            put(ctx, cells, wx, row, id('O'), if lit { 0.95 } else { 0.5 });
        }
    }
    // A balcony part way up.
    let balcony = base - height * 2 / 3;
    for x in -half - 2..=half + 2 {
        put(ctx, cells, tcol + lean + x, balcony, id('='), 0.55);
    }
    // The roof: a tall cone, a star at its tip.
    let roof_rows = ctx.scaled(10.0 + 4.0 * pick(6)).max(4);
    let roof_base = base - height;
    for r in 0..roof_rows {
        let row = roof_base - r;
        let reach = ((half + 2) * (roof_rows - r) / roof_rows).max(0);
        let c = top_col + lean * (r / 3);
        if reach == 0 {
            put(ctx, cells, c, row, id('^'), 0.6);
            continue;
        }
        put(ctx, cells, c - reach, row, id('/'), 0.55);
        put(ctx, cells, c + reach, row, id('\\'), 0.55);
        for x in c - reach + 1..c + reach {
            put(ctx, cells, x, row, id('^'), 0.42);
        }
    }
    let tip_col = top_col + lean * (roof_rows / 3);
    let tip_row = roof_base - roof_rows;
    put(ctx, cells, tip_col, tip_row, id('*'), 0.95);
    let y = co.ny(tip_row.max(0) as u32) - 0.5 / rows;
    for c in (tcol - half - 4).max(0)..=(tip_col + half + 4).min(i64::from(ctx.cols) - 1) {
        let s = &mut sky[c as usize];
        *s = s.min(y);
    }
    // A spiral of sparks around the roof and the top of the tower.
    let turns = 2.5 + pick(7);
    let steps = 40;
    for s in 0..steps {
        let t = f64::from(s) / f64::from(steps);
        let row = tip_row + 1 + (t * (roof_rows + 6) as f64) as i64;
        let x = sin(TAU * turns * t) * (half as f64 + 3.5 + 2.0 * t);
        let behind = sin(TAU * turns * t + TAU / 4.0) < 0.0;
        if behind {
            continue;
        }
        let k = hash3(seed, i64::from(s), 0x5A);
        let glyph = [id('*'), id('+'), id('.'), id('\'')][(k >> 30) as usize % 4];
        put(
            ctx,
            cells,
            top_col + x.round() as i64,
            row,
            glyph,
            0.6 + 0.35 * (1.0 - t),
        );
    }

    // Rocks floating in the air around the crag, trailing roots.
    for i in 0..3 {
        let k = hash3(seed, i, 0xF10);
        let nx = if group_left {
            0.5 + 0.45 * unit_from_hash(k)
        } else {
            0.05 + 0.45 * unit_from_hash(k)
        };
        let ny = 0.12 + 0.22 * unit_from_hash(k.rotate_left(17));
        let col = co.col_of(nx * w);
        if (col - tcol).abs() < 8 {
            continue;
        }
        let shape = if k & 1 == 0 { ISLET } else { ISLET_SMALL };
        let row = layers::row_at(ctx, ny);
        layers::stamp(ctx, cells, shape, col, row, 0.45);
        let top = co.ny((row - shape.len() as i64 + 1).max(0) as u32) - 0.5 / rows;
        for c in (col - 3).max(0)..=(col + 3).min(i64::from(ctx.cols) - 1) {
            // The stars stay off the rock (but go on below it).
            let s = &mut sky[c as usize];
            if *s > top {
                ctx.unsheltered.borrow_mut()[c as usize].get_or_insert(*s);
                *s = s.min(top);
            }
        }
    }

    let slopes = layers::paint(ctx, cells, &[SLOPES]);
    for (s, n) in sky.iter_mut().zip(slopes) {
        *s = s.min(n);
    }
    sky
}
