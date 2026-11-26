//! Windmills: flat country under a big sky, fields of flowers in long rows
//! running off to the horizon, a canal across them, and windmills with their
//! sails set standing along a dyke. The focal group stands on a grassy bank.

use super::layers::{self, Layer, put};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{TAU, sin};
use crate::recipe::TimeOfDay;

/// A low line of trees and farms along the horizon.
const TREES: Layer = Layer {
    family: &glyphs::FOLIAGE,
    hatch: None,
    base: -0.1,
    amp: 0.012,
    freq: 4.0,
    ridged: false,
    plateau: None,
    fill: (30.0, 30.0),
    threshold: 0.48,
    shade: 0.3,
    outline: Some(0.36),
    snow: None,
    label: "windmills_trees",
};

/// Where the fields start, above the horizon.
const FIELDS: f64 = 0.095;

/// What grows in each row of the fields, in turn.
const ROWS: [u8; 5] = [id('o'), id('.'), id('"'), id(':'), id('*')];

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "windmills");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let mut sky = layers::paint(ctx, cells, &[TREES]);
    let far = ctx.horizon - FIELDS;
    let far_row = layers::row_at(ctx, far);

    // The fields: rows converging on a point on the horizon.
    let vx = w * (0.3 + 0.4 * pick(0, 1));
    let fields = hash_label(seed, "fields");
    for row in far_row.max(0)..i64::from(ctx.rows) {
        let ny = co.ny(row as u32);
        let depth = (ny - far).max(1e-3);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row as u32);
            let px = co.nx(col) * w;
            // Which row of flowers this is, and how far across it.
            let across = (px - vx) / (depth * co.h) * 0.6;
            let lane = across.floor();
            let f = across - lane;
            let k = hash3(fields, col as i64, row);
            let glyph = if f < 0.18 {
                // The path between rows.
                if unit_from_hash(k) < 0.2 {
                    id('.')
                } else {
                    EMPTY
                }
            } else {
                let kind = ROWS[(lane as i64).rem_euclid(ROWS.len() as i64) as usize];
                // Sparse far off, thick close by.
                let keep = 0.35 + 0.6 * (depth / 0.3).min(1.0);
                if unit_from_hash(k) < keep {
                    kind
                } else {
                    EMPTY
                }
            };
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(0.26 + 0.25 * (depth / 0.4).min(1.0)),
            };
        }
    }

    // A canal across the fields, and the dyke behind it.
    let canal = far_row + ctx.scaled(4.0).max(2);
    for c in 0..i64::from(ctx.cols) {
        put(ctx, cells, c, canal - 1, id('_'), 0.4);
        put(
            ctx,
            cells,
            c,
            canal,
            if c % 3 == 0 { id('~') } else { id('-') },
            0.38,
        );
        put(ctx, cells, c, canal + 1, EMPTY, 0.0);
    }

    // The windmills along the dyke, the nearest biggest, away from the group.
    let mills = 2 + (pick(0, 2) * 2.0) as i64;
    let mut placed: Vec<f64> = Vec::new();
    for m in 0..mills {
        let mut x = w * (0.08 + 0.84 * pick(m, 3));
        if (x - ctx.group_cx).abs() < w * 0.15 {
            x = (x + w * 0.35) % w;
        }
        if placed.iter().any(|p| (p - x).abs() < w * 0.12) {
            continue;
        }
        placed.push(x);
        let scale = 0.6 + 0.5 * pick(m, 4);
        let top = mill(
            ctx,
            cells,
            co.col_of(x),
            far_row - 1,
            scale,
            pick(m, 5),
            lit(ctx.tod),
        );
        let c = co.col_of(x);
        let reach = (ctx.scaled(13.0 * scale) * 2).max(6);
        let y = co.ny(top.max(0) as u32) - 0.5 / rows;
        for x in (c - reach).max(0)..=(c + reach).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, x, y);
        }
    }
    sky
}

/// A windmill with its foot on `row`, its sails turned to `turn` (0..1 of a
/// quarter turn). Returns the highest row it reaches.
fn mill(
    ctx: &Ctx,
    cells: &mut [Cell],
    col: i64,
    row: i64,
    scale: f64,
    turn: f64,
    lit: bool,
) -> i64 {
    let tall = ctx.scaled(16.0 * scale).max(6);
    // The tower, wider at the foot.
    for k in 0..tall {
        let half = 2 + (3 * (tall - k) / tall);
        let r = row - k;
        let wall_l = if k > 0 && half != 2 + (3 * (tall - k + 1) / tall) {
            id('/')
        } else {
            id('|')
        };
        let wall_r = if wall_l == id('/') { id('\\') } else { id('|') };
        put(ctx, cells, col - half, r, wall_l, 0.55);
        put(ctx, cells, col + half, r, wall_r, 0.55);
        for c in col - half + 1..col + half {
            let glyph = if k < 2 && c == col {
                id('|')
            } else if k == tall / 2 && c == col {
                if lit { id('o') } else { id('#') }
            } else {
                id('#')
            };
            put(
                ctx,
                cells,
                c,
                r,
                glyph,
                if glyph == id('o') { 0.95 } else { 0.42 },
            );
        }
    }
    // The cap, and the hub the sails turn on.
    let hub = row - tall - 1;
    for c in col - 2..=col + 2 {
        put(ctx, cells, c, hub + 1, id('^'), 0.55);
    }
    put(ctx, cells, col, hub, id('o'), 0.65);
    // Four sails: a spar and a lattice beside it.
    let len = ctx.scaled(13.0 * scale).max(5) as f64;
    let mut top = hub;
    for s in 0..4 {
        let a = TAU * (turn / 4.0 + f64::from(s) / 4.0);
        let (dx, dy) = (sin(a + TAU / 4.0), sin(a));
        let spar = if (dx * dy) > 0.0 { id('\\') } else { id('/') };
        let spar = if dx.abs() < 0.3 {
            id('|')
        } else if dy.abs() < 0.3 {
            id('-')
        } else {
            spar
        };
        for k in 1..=len as i64 {
            let t = k as f64;
            // Cells are about twice as tall as wide.
            let c = col + (dx * t * 2.0).round() as i64;
            let r = hub + (dy * t).round() as i64;
            put(ctx, cells, c, r, spar, 0.6);
            if k > 2 {
                // The sail's lattice on one side of the spar.
                let (oc, or) = ((-dy * 2.0).round() as i64, (dx).round() as i64);
                put(
                    ctx,
                    cells,
                    c + oc,
                    r + or,
                    if k % 2 == 0 { id('#') } else { id('=') },
                    0.42,
                );
            }
            top = top.min(r - 1);
        }
    }
    top
}
