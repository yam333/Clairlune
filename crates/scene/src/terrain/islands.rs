//! Tropical islands: a calm sea to the horizon, small islets with palms,
//! the sun or the moon low over the water; the focal group stands on a
//! sandy spit.

use super::layers::{self, Layer};
use super::{Ctx, details, family_glyph, shade_u8};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::fbm2;

/// Water marks, light to heavy.
const SEA: [u8; 4] = [id('.'), id('-'), id('~'), id('=')];

/// The sea line, relative to the horizon.
const SEA_LINE: f64 = -0.06;

/// Islets: low humps, mostly under the sea line.
const ISLETS: Layer = Layer {
    family: &glyphs::SAND,
    hatch: None,
    base: SEA_LINE + 0.03,
    amp: 0.05,
    freq: 2.2,
    ridged: false,
    plateau: None,
    fill: (20.0, 18.0),
    threshold: 0.45,
    shade: 0.4,
    outline: Some(0.55),
    snow: None,
    label: "islands_islets",
};

const PALM: [&str; 6] = ["\\\\|//", "-\\|/-", "  |  ", "  |  ", "  |  ", "  |  "];
const PALM_LEAN: [&str; 6] = ["\\\\|//", "-\\|/-", "  /  ", "  |  ", " /   ", " |   "];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    if details::enabled(ctx) {
        // The desert's big low sun (or moon after dark), cut by the sea.
        details::desert_sky(ctx, cells);
    }
    let mut sky = layers::paint(ctx, cells, &[ISLETS]);
    let sea_top = ctx.horizon + SEA_LINE;
    // Palms on the islets that show above the water.
    let seed = hash_label(ctx.terrain, "islands_palms");
    let mut col = 2i64;
    while col < i64::from(ctx.cols) - 3 {
        let k = hash3(seed, col, 0);
        let nx = ctx.co.nx(col as u32);
        let top = ISLETS.top(ctx, nx);
        if top < sea_top - 0.012 && unit_from_hash(k) < 0.6 {
            let shape: &[&str] = if k & 1 == 0 { &PALM } else { &PALM_LEAN };
            let row = layers::row_at(ctx, top);
            layers::stamp(ctx, cells, shape, col, row, 0.5);
            let y = ctx.co.ny((row - shape.len() as i64).max(0) as u32);
            for c in (col - 2).max(0)..=(col + 2).min(i64::from(ctx.cols) - 1) {
                let s = &mut sky[c as usize];
                *s = s.min(y);
            }
            col += 8;
        }
        col += 3 + (k >> 40) as i64 % 4;
    }
    // The sea over everything below its line, but the islets.
    let sea = hash_label(ctx.terrain, "islands_sea");
    let first = (0..ctx.rows)
        .find(|&r| ctx.co.ny(r) >= sea_top)
        .unwrap_or(ctx.rows);
    let span = f64::from(ctx.rows - first).max(1.0);
    for row in first..ctx.rows {
        let t = f64::from(row - first) / span;
        let ny = ctx.co.ny(row);
        for col in 0..ctx.cols {
            let nx = ctx.co.nx(col);
            let idx = ctx.idx(col, row);
            if ISLETS.top(ctx, nx) < ny && ny < sea_top + 0.03 && cells[idx] != Cell::default() {
                continue; // the islet's foot, at the waterline
            }
            if row == first {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.4),
                };
                continue;
            }
            let n = fbm2(
                sea,
                nx * ctx.aspect * (6.0 + 12.0 * t),
                ny * (80.0 + 40.0 * t),
                3,
            );
            let d = clamp01((n - 0.5) * 2.2 + 0.5);
            let threshold = 0.74 - 0.2 * t;
            cells[idx] = if d < threshold {
                Cell::default()
            } else {
                Cell {
                    glyph: family_glyph(&SEA, d, threshold, sea, col, row),
                    shade: shade_u8(0.28 + 0.3 * t),
                }
            };
        }
    }
    for s in &mut sky {
        *s = s.min(sea_top);
    }
    sky
}
