//! Meadow: soft hills far off, rolling grass coming nearer, wildflowers in
//! it and a lone tree on a far rise; the focal group stands on a grassy
//! knoll.

use super::layers::{self, Layer};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};

/// Grass, light to heavy.
const GRASS: [u8; 4] = [id('.'), id(','), id('\''), id('"')];

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.12,
    amp: 0.06,
    freq: 1.1,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.28,
    outline: Some(0.45),
    snow: None,
    label: "meadow_hills",
};
const ROLLING: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: -0.01,
    amp: 0.05,
    freq: 1.7,
    ridged: false,
    plateau: None,
    fill: (20.0, 26.0),
    threshold: 0.5,
    shade: 0.45,
    outline: Some(0.55),
    snow: None,
    label: "meadow_rolling",
};
const NEAR: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: 0.09,
    amp: 0.035,
    freq: 2.4,
    ridged: false,
    plateau: None,
    fill: (16.0, 22.0),
    threshold: 0.42,
    shade: 0.62,
    outline: Some(0.7),
    snow: None,
    label: "meadow_near",
};

/// A broad tree standing alone on a far rise.
const TREE: [&str; 6] = [
    "  &&&&&  ",
    " &&%%%&& ",
    "&&%%@%%&&",
    " &&%%%&& ",
    "    |    ",
    "    |    ",
];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = layers::paint(ctx, cells, &[HILLS, ROLLING, NEAR]);
    // Wildflowers through the grass, thicker toward the viewer.
    let seed = hash_label(ctx.terrain, "meadow_flowers");
    const FLOWERS: [u8; 3] = [id('*'), id('o'), id('*')];
    for row in 0..ctx.rows {
        let ny = ctx.co.ny(row);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            if ny < sky[col as usize] + 0.02 || cells[idx] == Cell::default() {
                continue;
            }
            let near = ((ny - ctx.horizon) / 0.3).clamp(0.0, 1.0);
            let k = hash3(seed, i64::from(col), i64::from(row));
            if unit_from_hash(k) < 0.015 + 0.05 * near {
                cells[idx] = Cell {
                    glyph: FLOWERS[(k >> 33) as usize % FLOWERS.len()],
                    shade: shade_u8(0.55 + 0.3 * near),
                };
            }
        }
    }
    // The lone tree, away from the focal group.
    let k = hash3(seed, 1, 2);
    let left = ctx.group_cx > ctx.co.w * 0.5;
    let at = if left { 0.12 } else { 0.62 } + 0.22 * unit_from_hash(k);
    let col = ctx.co.col_of(at * ctx.co.w);
    if col > 4 && col < i64::from(ctx.cols) - 5 {
        let ny = rise_top(ctx, at);
        let row = layers::row_at(ctx, ny) + 1;
        layers::stamp(ctx, cells, &TREE, col, row, 0.42);
        let top = row - TREE.len() as i64;
        let y = ctx.co.ny(top.max(0) as u32) - 0.5 / f64::from(ctx.rows);
        for c in (col - 5).max(0)..=(col + 5).min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
    }
    sky
}

/// The land's top (normalised y) at normalised x, under the tree.
fn rise_top(ctx: &Ctx, nx: f64) -> f64 {
    HILLS.top(ctx, nx).min(ROLLING.top(ctx, nx))
}
