//! Swamp: still dark water among cypress trees hung with moss, their
//! reflections broken by lily pads, reeds and cypress knees at the edges;
//! the focal group stands on a mossy bank.

use super::layers::{self, Layer};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;

/// The far treeline across the water.
const TREELINE: Layer = Layer {
    family: &glyphs::FOLIAGE,
    hatch: None,
    base: -0.08,
    amp: 0.04,
    freq: 3.2,
    ridged: false,
    plateau: None,
    fill: (30.0, 30.0),
    threshold: 0.42,
    shade: 0.32,
    outline: Some(0.42),
    snow: None,
    label: "swamp_treeline",
};

/// The water begins this far below the horizon.
const WATER_LINE: f64 = -0.04;

const CYPRESS: [&str; 10] = [
    "   &%&   ",
    "  &%&%&  ",
    " &%;%&%& ",
    "&%&%;%&%&",
    " ;!|;|!; ",
    "  ! | !  ",
    "  ; | ;  ",
    "    |    ",
    "    |    ",
    "   /|\\   ",
];
const CYPRESS_SMALL: [&str; 6] = [
    "  &%&  ", " &%;%& ", " ;!|!; ", "  ;|;  ", "   |   ", "  /|\\  ",
];
const REEDS: [&str; 3] = ["\\|/", " | ", " | "];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "swamp");
    let mut sky = layers::paint(ctx, cells, &[TREELINE]);
    let water_top = ctx.horizon + WATER_LINE;
    let first = (0..ctx.rows)
        .find(|&r| co.ny(r) >= water_top)
        .unwrap_or(ctx.rows);
    // Cypresses standing in the water, painted before the mirror so they
    // are reflected; clear of the group.
    let mut col = 2i64;
    let trees = hash_label(seed, "cypress");
    while col < i64::from(ctx.cols) - 2 {
        let k = hash3(trees, col, 4);
        let nx = co.nx(col as u32);
        if (nx * w - ctx.group_cx).abs() > w * 0.12 && unit_from_hash(k) < 0.45 {
            let shape: &[&str] = if unit_from_hash(k.rotate_left(9)) < 0.45 {
                &CYPRESS
            } else {
                &CYPRESS_SMALL
            };
            // Standing a little into the water.
            let row = i64::from(first) + (k >> 30) as i64 % 2;
            layers::stamp(ctx, cells, shape, col, row, 0.46);
            let top = row - shape.len() as i64 + 1;
            let y = co.ny(top.max(0) as u32) - 0.5 / rows;
            let half = shape[0].len() as i64 / 2;
            for c in (col - half).max(0)..=(col + half).min(i64::from(ctx.cols) - 1) {
                let s = &mut sky[c as usize];
                *s = s.min(y);
            }
        }
        col += 7 + (k >> 50) as i64 % 9;
    }
    // The water: a dark mirror of what stands above it, lily pads on it.
    let water = hash_label(seed, "water");
    let above: Vec<Cell> = cells.to_vec();
    for row in first..ctx.rows {
        let depth = row - first;
        let mirror = first.checked_sub(depth + 1);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            let k = hash3(water, i64::from(col), i64::from(row));
            if row == first {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.38),
                };
                continue;
            }
            // Lily pads, more of them nearer.
            let near = f64::from(depth) / f64::from((ctx.rows - first).max(1));
            if unit_from_hash(k) < 0.016 + 0.04 * near {
                cells[idx] = Cell {
                    glyph: if (k >> 20).is_multiple_of(5) {
                        id('*')
                    } else {
                        id('o')
                    },
                    shade: shade_u8(0.5),
                };
                continue;
            }
            let ripple = fbm2(water, co.nx(col) * 30.0, f64::from(row) * 0.5, 2);
            let src = mirror.map(|m| above[ctx.idx(col, m)]);
            cells[idx] = match src {
                Some(c)
                    if c.glyph != EMPTY
                        && ripple < 0.6
                        && unit_from_hash(k.rotate_left(7)) < 0.75 =>
                {
                    Cell {
                        glyph: c.glyph,
                        shade: shade_u8(f64::from(c.shade) / 255.0 * 0.45),
                    }
                }
                _ if unit_from_hash(k.rotate_left(13)) < 0.04 => Cell {
                    glyph: id('-'),
                    shade: shade_u8(0.26),
                },
                _ => Cell::default(),
            };
        }
    }
    for s in &mut sky {
        *s = s.min(water_top);
    }
    // Reeds and cypress knees along the near water, clear of the group.
    let edge = hash_label(seed, "edge");
    let g0 = co.col_of(f64::from(ctx.gx0)) - 4;
    let g1 = co.col_of(f64::from(ctx.gx1)) + 4;
    let bottom = i64::from(ctx.rows) - 1;
    let mut col = 1i64;
    while col < i64::from(ctx.cols) - 1 {
        let k = hash3(edge, col, 0);
        if (col < g0 || col > g1) && unit_from_hash(k) < 0.4 {
            let row = bottom - (k >> 40) as i64 % 4;
            if k & 3 == 0 {
                layers::put(ctx, cells, col, row, id('^'), 0.5);
                layers::put(ctx, cells, col + 1, row, id('^'), 0.45);
            } else {
                layers::stamp(ctx, cells, &REEDS, col, row, 0.55);
            }
        }
        col += 3 + (k >> 50) as i64 % 5;
    }
    sky
}
