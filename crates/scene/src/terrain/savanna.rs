//! Savanna: open golden grassland to a far, flat-topped hill, umbrella
//! acacias standing apart, tall grass in front; the focal group stands on a
//! low bank of earth.

use super::Ctx;
use super::layers::{self, Layer};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};

/// Table hills far off: only the highest swells are lifted into flat tops,
/// with low ground between them.
const MESA: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.035,
    amp: 0.06,
    freq: 1.3,
    ridged: false,
    plateau: Some(0.78),
    fill: (24.0, 16.0),
    threshold: 0.5,
    shade: 0.28,
    outline: Some(0.42),
    snow: None,
    label: "savanna_mesa",
};

/// Dry grass, light to heavy.
const DRY: [u8; 4] = [id('.'), id(','), id(':'), id('\'')];
/// Tall grass, light to heavy.
const TALL: [u8; 4] = [id(','), id('\''), id('"'), id('!')];

const PLAIN: Layer = Layer {
    family: &DRY,
    hatch: None,
    base: -0.012,
    amp: 0.012,
    freq: 1.4,
    ridged: false,
    plateau: None,
    fill: (26.0, 30.0),
    threshold: 0.48,
    shade: 0.42,
    outline: Some(0.5),
    snow: None,
    label: "savanna_plain",
};

const NEAR: Layer = Layer {
    family: &TALL,
    hatch: None,
    base: 0.1,
    amp: 0.035,
    freq: 3.0,
    ridged: false,
    plateau: None,
    fill: (30.0, 14.0),
    threshold: 0.4,
    shade: 0.6,
    outline: None,
    snow: None,
    label: "savanna_near",
};

const ACACIA: [&str; 6] = [
    " &&%&&&%&&& ",
    "&%&&%&&%&&%&",
    "   \\  | /   ",
    "    \\ |/    ",
    "     \\|     ",
    "      |     ",
];
const ACACIA_SMALL: [&str; 4] = [" &&%&&& ", "&%&&%&&%", "  \\ |/  ", "    |   "];
const ACACIA_FAR: [&str; 2] = ["&&%&", " |  "];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "savanna");
    let mut sky = layers::paint(ctx, cells, &[MESA, PLAIN]);
    let shelter = |sky: &mut [f64], c0: i64, c1: i64, top_row: i64| {
        let y = co.ny(top_row.max(0) as u32) - 0.5 / rows;
        for c in c0.max(0)..=c1.min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
    };
    // Acacias: one or two big ones nearer, smaller ones far off on the plain.
    let trees = hash_label(seed, "acacias");
    let mut col = 3i64;
    let mut big = 0;
    while col < i64::from(ctx.cols) - 3 {
        let k = hash3(trees, col, 9);
        let nx = co.nx(col as u32);
        let clear = (nx * w - ctx.group_cx).abs() > w * 0.13;
        if clear && unit_from_hash(k) < 0.4 {
            let r = unit_from_hash(k.rotate_left(11));
            let (shape, shade, sink): (&[&str], f64, i64) = if r < 0.3 && big < 2 {
                big += 1;
                (&ACACIA, 0.5, 2)
            } else if r < 0.65 {
                (&ACACIA_SMALL, 0.44, 1)
            } else {
                (&ACACIA_FAR, 0.36, 0)
            };
            let row = layers::row_at(ctx, PLAIN.top(ctx, nx)) + sink;
            layers::stamp(ctx, cells, shape, col, row, shade);
            let half = shape[0].len() as i64 / 2;
            shelter(
                &mut sky,
                col - half,
                col + half,
                row - shape.len() as i64 + 1,
            );
        }
        col += 8 + (k >> 40) as i64 % 10;
    }
    // A few termite mounds and stones in the grass.
    let bits = hash_label(seed, "mounds");
    for row in 0..ctx.rows {
        let ny = co.ny(row);
        if ny < ctx.horizon + 0.01 {
            continue;
        }
        for col in 0..ctx.cols {
            let k = hash3(bits, i64::from(col), i64::from(row));
            if unit_from_hash(k) < 0.004 && (co.nx(col) * w - ctx.group_cx).abs() > w * 0.1 {
                layers::put(
                    ctx,
                    cells,
                    i64::from(col),
                    i64::from(row),
                    if k & 1 == 0 { id('^') } else { id('o') },
                    0.5,
                );
            }
        }
    }
    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}
