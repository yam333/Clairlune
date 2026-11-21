//! Bamboo grove: rows of tall stalks with knotted joints and sprays of
//! leaves, fading into the haze behind; the focal group stands on a mossy
//! bank in a clearing.

use super::Ctx;
use super::layers::{self, Layer};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};

const FLOOR: Layer = Layer {
    family: &glyphs::EARTH,
    hatch: None,
    base: 0.05,
    amp: 0.015,
    freq: 1.6,
    ridged: false,
    plateau: None,
    fill: (26.0, 22.0),
    threshold: 0.6,
    shade: 0.4,
    outline: Some(0.5),
    snow: None,
    label: "bamboo_floor",
};

/// One row of stalks: spacing in columns, shade, and where it stands.
struct Row {
    spacing: (i64, i64),
    shade: f64,
    /// Where the stalks stand, below the horizon (normalised).
    ground: f64,
    label: &'static str,
}

const ROWS: [Row; 3] = [
    Row {
        spacing: (7, 11),
        shade: 0.14,
        ground: -0.02,
        label: "bamboo_far",
    },
    Row {
        spacing: (10, 15),
        shade: 0.28,
        ground: 0.02,
        label: "bamboo_mid",
    },
    Row {
        spacing: (16, 26),
        shade: 0.55,
        ground: 0.07,
        label: "bamboo_near",
    },
];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = layers::paint(ctx, cells, &[FLOOR]);
    // A clearing around the group: the nearer rows step aside.
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - 4;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + 4;
    for (ri, row) in ROWS.iter().enumerate() {
        let seed = hash_label(ctx.terrain, row.label);
        let ground = layers::row_at(ctx, ctx.horizon + row.ground);
        let mut col = (hash3(seed, 0, 1) % 4) as i64;
        let mut i = 0i64;
        while col < i64::from(ctx.cols) {
            i += 1;
            let k = hash3(seed, i, 0);
            if ri > 0 && (g0..=g1).contains(&col) {
                col += 1;
                continue;
            }
            // Joints every few rows, staggered per stalk.
            let joint = 4 + (k >> 8) as i64 % 3;
            let offset = (k >> 16) as i64 % joint;
            for r in 0..=ground {
                let glyph = if (r + offset) % joint == 0 {
                    id('+')
                } else {
                    id('|')
                };
                layers::put(ctx, cells, col, r, glyph, row.shade);
                // A spray of leaves off some joints.
                if (r + offset) % joint == 0 && unit_from_hash(hash3(seed, i, r)) < 0.4 {
                    let side = if unit_from_hash(hash3(seed, i, r + 1)) < 0.5 {
                        -1
                    } else {
                        1
                    };
                    let leaf = if side < 0 { id('\\') } else { id('/') };
                    layers::put(ctx, cells, col + side, r - 1, leaf, row.shade);
                    layers::put(ctx, cells, col + 2 * side, r - 1, id('-'), row.shade * 0.9);
                }
            }
            if (0..i64::from(ctx.cols)).contains(&col) {
                sky[col as usize] = 0.0;
            }
            col += row.spacing.0 + (k >> 40) as i64 % (row.spacing.1 - row.spacing.0 + 1);
        }
    }
    sky
}
