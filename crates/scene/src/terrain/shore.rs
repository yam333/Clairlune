//! Shore: open sea to the horizon with islands breaking the sea line, water
//! streaks that lengthen and thicken toward the viewer, and a wooden pier
//! running in from the nearer screen edge for the focal group to stand on.

use super::layers::{self, Layer};
use super::{Ctx, details, family_glyph, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3};
use crate::math::{clamp01, pm};
use crate::noise::fbm2;

/// Water marks, light to heavy.
const SEA: [u8; 4] = [id('.'), id('-'), id('~'), id('=')];

/// The sea line, normalised, relative to the horizon (the pier deck).
const SEA_LINE: f64 = -0.07;

/// Islands: a smooth layer mostly below the sea line, so only its highest
/// swells show above the water.
const ISLANDS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: SEA_LINE + 0.025,
    amp: 0.11,
    freq: 1.2,
    ridged: false,
    plateau: None,
    fill: (26.0, 18.0),
    threshold: 0.45,
    shade: 0.3,
    outline: Some(0.45),
    snow: None,
    label: "shore_islands",
};

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = layers::paint(ctx, cells, &[ISLANDS]);
    let sea_top = ctx.horizon + SEA_LINE;
    let seed = hash_label(ctx.terrain, "shore_sea");
    let first = (0..ctx.rows)
        .find(|&r| ctx.co.ny(r) >= sea_top)
        .unwrap_or(ctx.rows);
    let span = f64::from(ctx.rows - first).max(1.0);
    for row in first..ctx.rows {
        // 0 at the sea line, 1 at the bottom of the screen.
        let t = f64::from(row - first) / span;
        let ny = ctx.co.ny(row);
        for col in 0..ctx.cols {
            let nx = ctx.co.nx(col);
            let idx = ctx.idx(col, row);
            if row == first {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.42),
                };
                continue;
            }
            // Long streaks far off, shorter and busier up close.
            let n = fbm2(
                seed,
                nx * ctx.aspect * (5.0 + 14.0 * t),
                ny * (90.0 + 40.0 * t),
                3,
            );
            let d = clamp01((n - 0.5) * 2.2 + 0.5);
            let threshold = 0.70 - 0.22 * t;
            cells[idx] = if d < threshold {
                Cell::default()
            } else {
                let glyph = family_glyph(&SEA, d, threshold, seed, col, row);
                let mut shade = 0.3 + 0.35 * t;
                if row > ctx.platform_row {
                    shade *= 0.8;
                }
                Cell {
                    glyph,
                    shade: shade_u8(shade),
                }
            };
        }
    }
    for s in &mut sky {
        *s = s.min(sea_top);
    }
    if details::enabled(ctx) {
        details::shore_life(ctx, cells, &mut sky, sea_top);
    }
    sky
}

/// The pier: a plank deck on the platform row from the nearer screen edge to
/// just past the focal group, posts every few columns down into the water,
/// shadow under the deck, and a taller mooring post at the sea end.
pub(crate) fn pier(ctx: &Ctx, cells: &mut [Cell]) {
    let co = &ctx.co;
    let comp = &ctx.recipe.composition;
    let margin_px = pm(comp.platform_margin_pm) * ctx.unit + 2.0 * f64::from(co.g.cell_w);
    let pc0 = co.col_of(f64::from(ctx.gx0) - margin_px).max(0);
    let pc1 = co
        .col_of(f64::from(ctx.gx1) + margin_px)
        .min(i64::from(ctx.cols) - 1);
    let from_left = ctx.group_cx < co.w * 0.5;
    let (c0, c1, sea_end) = if from_left {
        (0, pc1, pc1)
    } else {
        (pc0, i64::from(ctx.cols) - 1, pc0)
    };
    let deck = i64::from(ctx.platform_row);
    let water = deck + 2;
    let depth = 4;
    let seed = hash_label(ctx.terrain, "pier");
    // Deck planks, with a worn plank here and there away from the group.
    for c in c0..=c1 {
        let k = hash3(seed, c, 0);
        let worn = k.is_multiple_of(11) && !(pc0..=pc1).contains(&c);
        let glyph = if worn { id('-') } else { id('=') };
        layers::put(ctx, cells, c, deck, glyph, 0.85);
        // Shadow under the deck, down to the waterline.
        for r in deck + 1..water {
            layers::put(ctx, cells, c, r, EMPTY, 0.0);
        }
    }
    // Posts from the sea end inward, every 6 columns.
    let step = if from_left { -6 } else { 6 };
    let mut c = sea_end;
    while (c0..=c1).contains(&c) {
        for r in deck + 1..water + depth {
            let glyph = if r == water { id('#') } else { id('|') };
            layers::put(ctx, cells, c, r, glyph, 0.6);
        }
        // Ripples where the post meets the water.
        for dc in [-1, 1] {
            layers::put(ctx, cells, c + dc, water, id('~'), 0.5);
        }
        c += step;
    }
    // A mooring post standing above the deck at the sea end.
    layers::put(ctx, cells, sea_end, deck - 1, id('|'), 0.7);
    layers::put(ctx, cells, sea_end, deck - 2, id('o'), 0.7);
}
