//! Lake: mountains and wooded hills across still water, mirrored in it,
//! reeds at the near edge; the focal group stands on a grassy bank.

use super::layers::{self, Layer, Snow};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;

const SNOWCAP: [u8; 3] = [id('.'), id(':'), id('\'')];

const PEAKS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: Some((&glyphs::FAR_LEFT, &glyphs::FAR_RIGHT)),
    base: -0.2,
    amp: 0.12,
    freq: 1.2,
    ridged: true,
    plateau: None,
    fill: (26.0, 14.0),
    threshold: 0.48,
    shade: 0.32,
    outline: Some(0.5),
    snow: Some(Snow {
        line: 0.2,
        rows: 2.0,
        glyphs: &SNOWCAP,
        shade: 0.55,
    }),
    label: "lake_peaks",
};
const WOODS: Layer = Layer {
    family: &glyphs::FOLIAGE,
    hatch: None,
    base: -0.075,
    amp: 0.03,
    freq: 3.0,
    ridged: false,
    plateau: None,
    fill: (30.0, 30.0),
    threshold: 0.4,
    shade: 0.38,
    outline: Some(0.45),
    snow: None,
    label: "lake_woods",
};

/// The far shore, where the water begins (normalised, from the horizon).
const WATER_LINE: f64 = -0.05;

const REED: [&str; 3] = ["\\|/", " | ", " | "];
const REED_SMALL: [&str; 2] = ["|/", "| "];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = layers::paint(ctx, cells, &[PEAKS, WOODS]);
    let water_top = ctx.horizon + WATER_LINE;
    let first = (0..ctx.rows)
        .find(|&r| ctx.co.ny(r) >= water_top)
        .unwrap_or(ctx.rows);
    let seed = hash_label(ctx.terrain, "lake_water");
    // The land above, as painted, for the mirror.
    let above: Vec<Cell> = cells.to_vec();
    for row in first..ctx.rows {
        let depth = row - first;
        // The reflection of the row as far above the waterline.
        let mirror = first.checked_sub(depth + 1);
        let fade = 1.0 - (f64::from(depth) / f64::from((ctx.rows - first).max(1))).min(1.0);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            if row == first {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.45),
                };
                continue;
            }
            // A ripple: the reflection shifts a cell now and then.
            let ripple = fbm2(seed, ctx.co.nx(col) * 40.0, f64::from(row) * 0.7, 2);
            let shift = if ripple > 0.62 { 1 } else { 0 };
            let src = mirror.map(|m| above[ctx.idx((col + shift).min(ctx.cols - 1), m)]);
            let k = hash3(seed, i64::from(col), i64::from(row));
            cells[idx] = match src {
                Some(c) if c.glyph != EMPTY && unit_from_hash(k) < 0.8 => Cell {
                    // Mirrored ink, softer, broken by the water.
                    glyph: if ripple > 0.58 { id('~') } else { c.glyph },
                    shade: shade_u8(f64::from(c.shade) / 255.0 * (0.45 + 0.15 * fade)),
                },
                _ if unit_from_hash(k >> 7) < 0.05 + 0.08 * (1.0 - fade) => Cell {
                    glyph: if unit_from_hash(k >> 21) < 0.5 {
                        id('-')
                    } else {
                        id('~')
                    },
                    shade: shade_u8(0.3),
                },
                _ => Cell::default(),
            };
        }
    }
    for s in &mut sky {
        *s = s.min(water_top);
    }
    // Reeds along the near water, clear of the group.
    let reeds = hash_label(ctx.terrain, "lake_reeds");
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - 4;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + 4;
    let bottom = i64::from(ctx.rows) - 1;
    let mut col = 1i64;
    while col < i64::from(ctx.cols) - 1 {
        let k = hash3(reeds, col, 0);
        if (col < g0 || col > g1) && unit_from_hash(k) < 0.35 {
            let shape: &[&str] = if k & 1 == 0 { &REED } else { &REED_SMALL };
            let row = bottom - (k >> 40) as i64 % 3;
            layers::stamp(ctx, cells, shape, col, row, 0.6);
        }
        col += 3 + (k >> 50) as i64 % 4;
    }
    sky
}
