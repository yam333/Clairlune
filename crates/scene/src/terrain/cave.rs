//! Cave: no sky. A rock arch frames a dim back wall; stalactites hang from the
//! ceiling, stalagmites rise from the far floor, crystal clusters glint on the
//! walls; the focal group stands on a rock ledge.

use super::layers::{self, Layer, put, stamp};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::{fbm1, fbm2};

const FLOOR: Layer = Layer {
    family: &glyphs::NEAR,
    hatch: None,
    base: 0.02,
    amp: 0.015,
    freq: 2.5,
    ridged: true,
    plateau: None,
    fill: (20.0, 16.0),
    threshold: 0.55,
    shade: 0.4,
    outline: Some(0.5),
    snow: None,
    label: "cave_floor",
};

const STALAGMITES: [&[&str]; 3] = [&["^", "|"], &["^", "/|\\"], &["^", "|", "/|\\"]];
const CRYSTALS: [&[&str]; 3] = [&["*"], &["+*", "x+"], &[" * ", "*+x"]];

fn pick(seed: u64, a: i64, b: i64) -> f64 {
    unit_from_hash(hash3(seed, a, b))
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let seed = hash_label(ctx.terrain, "cave");
    let rows = i64::from(ctx.rows);
    // The arch: rock above `ceiling[col]`, open chamber below it.
    let cx = 0.5 + (pick(seed, 0, 0) - 0.5) * 0.2;
    let rx = 0.40 + 0.08 * pick(seed, 0, 1);
    let ry = ctx.horizon - 0.2 - 0.06 * pick(seed, 0, 2);
    let ceiling: Vec<f64> = (0..ctx.cols)
        .map(|col| {
            let nx = ctx.co.nx(col);
            let u = (nx - cx) / rx;
            let open = (1.0 - u * u).max(0.0).sqrt();
            ctx.horizon - ry * open + 0.05 * (fbm1(seed, nx * 7.0, 3) - 0.5)
        })
        .collect();
    let wall_seed = seed ^ 0xBACC;
    for row in 0..ctx.platform_row {
        let ny = ctx.co.ny(row);
        for col in 0..ctx.cols {
            let nx = ctx.co.nx(col);
            let c = ceiling[col as usize];
            let n = fbm2(wall_seed, nx * ctx.aspect * 18.0, ny * 14.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.5);
            let (family, threshold, shade): (&[u8], f64, f64) = if ny < c {
                // Rock: darker deeper in.
                let depth = ((c - ny) * 6.0).min(1.0);
                (&glyphs::NEAR, 0.45, 0.4 - 0.15 * depth)
            } else {
                (&glyphs::DISTANT, 0.7, 0.2)
            };
            if d < threshold {
                continue;
            }
            let glyph = family_glyph(family, d, threshold, wall_seed, col, row);
            put(ctx, cells, i64::from(col), i64::from(row), glyph, shade);
        }
    }
    // Stalactites: a few, hanging from the ceiling edge. Long ones get a wide
    // root (`\|/`), all end in a `v` tip.
    for col in 0..ctx.cols {
        let c = i64::from(col);
        if pick(seed, c, 1) > 0.07 || !clear_of_group(ctx, c, 2) {
            continue;
        }
        let edge = row_below(ctx, ceiling[col as usize]);
        let len = 1 + (pick(seed, c, 2) * 4.0) as i64;
        for r in edge..edge + len {
            put(ctx, cells, c, r, id('|'), 0.42);
        }
        put(ctx, cells, c, edge + len, id('v'), 0.48);
        if len >= 3 {
            put(ctx, cells, c - 1, edge, id('\\'), 0.42);
            put(ctx, cells, c + 1, edge, id('/'), 0.42);
        }
    }
    let mut sky = layers::paint(ctx, cells, &[FLOOR]);
    // Stalagmites on the far floor, clear of the focal group.
    let floor_top = ctx.horizon + FLOOR.base;
    for i in 0..10 {
        let nx = pick(seed, i, 3);
        let col = ctx.co.col_of(nx * ctx.co.w);
        if clear_of_group(ctx, col, 3) {
            let shape = STALAGMITES[(pick(seed, i, 4) * 3.0) as usize];
            let row = row_below(ctx, floor_top) - 1;
            stamp(ctx, cells, shape, col, row, 0.42);
        }
    }
    // Crystal clusters where the walls meet the chamber.
    for i in 0..7 {
        let col = (pick(seed, i, 5) * f64::from(ctx.cols)) as i64;
        let c = col.clamp(0, i64::from(ctx.cols) - 1) as usize;
        let row = row_below(ctx, ceiling[c]) - 1;
        if row < rows - 1 && clear_of_group(ctx, col, 4) {
            let shape = CRYSTALS[(pick(seed, i, 6) * 3.0) as usize];
            stamp(ctx, cells, shape, col, row, 0.9);
        }
    }
    // No sky: nothing for stars or daytime dust.
    sky.fill(f64::NEG_INFINITY);
    sky
}

/// The first row whose centre is at or below normalised y.
fn row_below(ctx: &Ctx, y: f64) -> i64 {
    let g = &ctx.co.g;
    ((y * ctx.co.h - f64::from(g.origin_y)) / f64::from(g.cell_h) - 0.5).ceil() as i64
}

/// Whether a stamp at `col` keeps `margin` columns away from the focal group.
fn clear_of_group(ctx: &Ctx, col: i64, margin: i64) -> bool {
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - margin;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + margin;
    col < g0 || col > g1
}
