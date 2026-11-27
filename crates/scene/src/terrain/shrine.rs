//! Hillside shrine: stone steps climbing a wooded slope through torii gates
//! to a gabled wooden hall near the crest, stone lanterns along the way lit
//! from golden hour through the night. The focal group stands on a rock ledge
//! on this side.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

const MISTY_RIDGES: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.21,
    amp: 0.06,
    freq: 1.3,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.52,
    shade: 0.22,
    outline: Some(0.34),
    snow: None,
    label: "shrine_ridges",
};

const WOODED_SLOPE: Layer = Layer {
    family: &glyphs::FOLIAGE,
    hatch: None,
    base: -0.04,
    amp: 0.045,
    freq: 1.8,
    ridged: false,
    plateau: None,
    fill: (20.0, 22.0),
    threshold: 0.46,
    shade: 0.36,
    outline: Some(0.46),
    snow: None,
    label: "shrine_slope",
};

const HALL: &[&str] = &[
    "    _/^\\_    ",
    "  _/=====\\_  ",
    " /=========\\ ",
    "  |##|o|##|  ",
    "  |##|_|##|  ",
];

const STONE_LANTERN: &[&str] = &[" ^ ", "=O=", " | ", "_#_"];

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::GoldenHour | TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

/// Paints a torii gate centred on `col` with its column bases on `foot_row`.
fn torii(ctx: &Ctx, cells: &mut [Cell], col: i64, foot_row: i64, half: i64, tall: i64) -> i64 {
    let top = foot_row - tall;
    // Curved top lintel (`kasagi`) with upturned ends, tie beam (`nuki`) two rows below.
    put(ctx, cells, col - half - 2, top - 1, id('\\'), 0.62);
    put(ctx, cells, col + half + 2, top - 1, id('/'), 0.62);
    for c in col - half - 1..=col + half + 1 {
        put(ctx, cells, c, top, id('='), 0.64);
    }
    for c in col - half..=col + half {
        put(ctx, cells, c, top + 2, id('-'), 0.56);
    }
    put(ctx, cells, col, top + 1, id('|'), 0.58);
    for r in top + 1..=foot_row {
        put(ctx, cells, col - half, r, id('|'), 0.6);
        put(ctx, cells, col + half, r, id('|'), 0.6);
    }
    top - 1
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "shrine");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x541E));
    let evening = lit(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[MISTY_RIDGES, WOODED_SLOPE]);

    // The shrine sits on a wooded knoll on the far side from the focal group,
    // with stone steps climbing toward it through two torii gates.
    let far = if ctx.group_cx < w * 0.5 { 0.7 } else { 0.3 };
    let top_x = w * (far + 0.06 * (pick(1) - 0.5));
    let dir = if ctx.group_cx < w * 0.5 { 1.0 } else { -1.0 };
    let foot_x = top_x - dir * w * (0.16 + 0.04 * pick(2));
    let crest_ny = ctx.horizon - (0.19 + 0.04 * pick(3));
    let crest_row = layers::row_at(ctx, crest_ny);
    let base_row = layers::row_at(ctx, ctx.horizon);
    let top_col = co.col_of(top_x);
    let foot_col = co.col_of(foot_x);

    // Cedar crowns flanking the shrine on the crest.
    let grove = hash_label(seed, "cedars");
    for side in [-1i64, 1] {
        let gc = top_col + side * ctx.scaled(13.0).max(8);
        let gw = ctx.scaled(10.0).max(5);
        let gh = ctx.scaled(12.0).max(6);
        let gr = crest_row - gh / 2;
        for r in gr - gh..=gr + gh / 2 {
            for c in gc - gw..=gc + gw {
                let u = (c - gc) as f64 / gw as f64;
                let v = (r - gr) as f64 / gh as f64;
                if u * u + v * v > 1.0 || c < 0 || c >= i64::from(ctx.cols) || r < 0 {
                    continue;
                }
                let n = fbm2(grove, c as f64 * 0.2, r as f64 * 0.28, 3);
                let d = clamp01((n - 0.5) * 1.9 + 0.68 - 0.3 * (u * u + v * v));
                if d > 0.36 {
                    let g = family_glyph(&glyphs::FOLIAGE, d, 0.36, grove, c as u32, r as u32);
                    put(ctx, cells, c, r, g, 0.48);
                }
            }
        }
        let y = co.ny((gr - gh).max(0) as u32);
        for c in (gc - gw).max(0)..=(gc + gw).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, c, y);
        }
    }

    // The shrine hall on the crest.
    layers::stamp(ctx, cells, HALL, top_col, crest_row, 0.56);
    if evening {
        put(ctx, cells, top_col, crest_row - 1, id('o'), 0.98);
    }
    let hall_top = co.ny((crest_row - HALL.len() as i64).max(0) as u32) - 0.5 / rows;
    for c in (top_col - 8).max(0)..=(top_col + 8).min(i64::from(ctx.cols) - 1) {
        super::shelter(ctx, &mut sky, c, hall_top);
    }

    // Stone steps climbing from `foot_col, base_row` to `top_col, crest_row + 1`.
    let steps = (base_row - crest_row).max(4);
    let half_stair = ctx.scaled(3.0).max(2);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let r = base_row - i;
        let c = (foot_col as f64 + (top_col - foot_col) as f64 * t).round() as i64;
        let w_step = half_stair - (t * 1.0) as i64;
        for x in c - w_step..=c + w_step {
            put(
                ctx,
                cells,
                x,
                r,
                if i % 2 == 0 { id('=') } else { id('-') },
                0.48,
            );
        }
    }

    // Two torii gates straddling the steps (larger near the foot, smaller halfway up)
    // and stone lanterns beside them.
    for (frac, scale) in [(0.15, 1.0), (0.6, 0.78)] {
        let r = base_row - (steps as f64 * frac).round() as i64;
        let c = (foot_col as f64 + (top_col - foot_col) as f64 * frac).round() as i64;
        let half = ctx.scaled(5.0 * scale).max(3);
        let tall = ctx.scaled(9.0 * scale).max(5);
        let gate_top = torii(ctx, cells, c, r, half, tall);
        let y = co.ny(gate_top.max(0) as u32) - 0.5 / rows;
        for x in (c - half - 2).max(0)..=(c + half + 2).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, x, y);
        }
        let lcol = c + (dir as i64) * (half + 4);
        layers::stamp(ctx, cells, STONE_LANTERN, lcol, r, 0.52);
        put(
            ctx,
            cells,
            lcol,
            r - 2,
            id('O'),
            if evening { 0.98 } else { 0.52 },
        );
    }

    sky
}
