//! Waterfall: a cliff wall across part of the view, a fall pouring from its
//! lip into a misty pool, wooded hills behind and grass in front; the focal
//! group stands on a rock ledge.

use super::layers::{self, Layer};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, smoothstep};
use crate::noise::{fbm1, fbm2};

const HILLS: Layer = Layer {
    family: &glyphs::FOLIAGE,
    hatch: None,
    base: -0.1,
    amp: 0.05,
    freq: 1.6,
    ridged: false,
    plateau: None,
    fill: (26.0, 26.0),
    threshold: 0.45,
    shade: 0.3,
    outline: Some(0.42),
    snow: None,
    label: "waterfall_hills",
};

/// Grass, light to heavy.
const GRASS: [u8; 4] = [id('.'), id(','), id('\''), id('"')];

const NEAR: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: 0.11,
    amp: 0.03,
    freq: 2.6,
    ridged: false,
    plateau: None,
    fill: (18.0, 24.0),
    threshold: 0.42,
    shade: 0.6,
    outline: Some(0.68),
    snow: None,
    label: "waterfall_near",
};

/// The cliff face, light to heavy: broken rock, so the water's strokes
/// stand out against it.
const FACE: [u8; 4] = [id('.'), id(':'), id('x'), id('%')];
/// Bushes along the cliff's lip.
const BUSH: &[&str] = &["&%&"];
const BUSH_SMALL: &[&str] = &["%&"];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    let seed = hash_label(ctx.terrain, "waterfall");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0xFA11));
    let mut sky = layers::paint(ctx, cells, &[HILLS]);

    // The cliff, on the far side from the group, running off that edge.
    let span = w * (0.34 + 0.14 * pick(1));
    let group_left = ctx.group_cx < w * 0.5;
    let (x0, x1) = if group_left {
        (w * 1.04 - span, w * 1.04)
    } else {
        (-0.04 * w, span - 0.04 * w)
    };
    let lip = ctx.horizon - (0.2 + 0.08 * pick(2));
    let pool = ctx.horizon + 0.035;
    let edge = w * 0.04;
    let cliff_top = |px: f64| -> Option<f64> {
        let inside =
            smoothstep(x0 - edge, x0 + edge, px) * (1.0 - smoothstep(x1 - edge, x1 + edge, px));
        if inside <= 0.03 {
            return None;
        }
        let n = (fbm1(seed, px / h * 7.0, 3) - 0.5) * 0.03;
        Some(pool - (pool - lip) * inside + n)
    };
    // The fall: a band of water from the lip to the pool, away from the
    // cliff's ends.
    let fx = x0 + (x1 - x0) * (0.38 + 0.24 * pick(3));
    let fw = w * (0.04 + 0.02 * pick(4));
    let face_seed = hash_label(seed, "face");
    let water_seed = hash_label(seed, "water");
    let rows = f64::from(ctx.rows);
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let Some(top) = cliff_top(px) else {
            continue;
        };
        sky[col as usize] = sky[col as usize].min(top);
        let u = (px - fx) / (fw * 0.5);
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < top {
                continue;
            }
            if ny >= pool {
                break;
            }
            let below = (ny - top) * rows;
            let idx = ctx.idx(col, row);
            if u.abs() <= 1.0 {
                // Falling water: bright streaks, softer at the band's edges.
                let k = hash3(water_seed, i64::from(col), i64::from(row));
                cells[idx] = if below < 1.0 {
                    Cell {
                        glyph: id('~'),
                        shade: shade_u8(0.85),
                    }
                } else if u.abs() > 0.7 {
                    Cell {
                        glyph: id('|'),
                        shade: shade_u8(0.55),
                    }
                } else {
                    let g = [id('|'), id('!'), id(':'), id('|')][(k >> 30) as usize % 4];
                    Cell {
                        glyph: g,
                        shade: shade_u8(0.75 + 0.2 * unit_from_hash(k)),
                    }
                };
                continue;
            }
            if below < 1.0 {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.6),
                };
                continue;
            }
            // Moss along the top, then rock with cracks running down.
            let k = hash3(face_seed, i64::from(col), i64::from(row));
            if below < 2.5 && unit_from_hash(k) < 0.55 {
                cells[idx] = Cell {
                    glyph: glyphs::FOLIAGE[(k >> 33) as usize % 3 + 1],
                    shade: shade_u8(0.5),
                };
                continue;
            }
            let depth = clamp01((ny - top) / (pool - lip).max(0.05));
            let n = fbm2(face_seed, px / h * 16.0, ny * 22.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.5);
            cells[idx] = if d < 0.4 {
                Cell::default()
            } else {
                let i = (((d - 0.4) / 0.6) * FACE.len() as f64) as usize;
                Cell {
                    glyph: FACE[i.min(FACE.len() - 1)],
                    shade: shade_u8(0.46 * (1.0 - 0.35 * depth)),
                }
            };
        }
    }

    // The pool below the fall, foam where the water lands, spray above.
    let pw = fw * 2.5 + (x1 - x0) * 0.2;
    let pool_rows = 0.055;
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let off = (px - fx).abs();
        // Its edge wobbles; it narrows toward the viewer.
        let reach = pw * (0.85 + 0.3 * fbm1(water_seed, px / h * 3.0, 2));
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            let idx = ctx.idx(col, row);
            let k = hash3(water_seed ^ 0x9001, i64::from(col), i64::from(row));
            // Spray: a mist hanging over where the water lands.
            if ny < pool && ny >= pool - 0.045 && off < fw * 1.8 && off > fw * 0.5 {
                if unit_from_hash(k) < 0.35 * (1.0 - off / (fw * 1.8)) {
                    cells[idx] = Cell {
                        glyph: [id('.'), id('\''), id('`')][(k >> 40) as usize % 3],
                        shade: shade_u8(0.6),
                    };
                }
                continue;
            }
            if ny < pool || ny > pool + pool_rows {
                continue;
            }
            let t = (ny - pool) / pool_rows;
            if off > reach * (1.0 - 0.35 * t) {
                continue;
            }
            let first = (ny - pool) * rows < 1.0;
            cells[idx] = if off < fw * 0.8 && t < 0.45 {
                Cell {
                    glyph: [id('o'), id('O'), id('.'), id('o')][(k >> 36) as usize % 4],
                    shade: shade_u8(0.82),
                }
            } else if first {
                Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.5),
                }
            } else {
                Cell {
                    glyph: [id('~'), id('-'), id('~'), id('=')][(k >> 36) as usize % 4],
                    shade: shade_u8(0.42 + 0.12 * (1.0 - t)),
                }
            };
        }
    }

    // Bushes along the lip, clear of the fall.
    let bush_seed = hash_label(seed, "bushes");
    let mut col = 0i64;
    while col < i64::from(ctx.cols) {
        let k = hash3(bush_seed, col, 1);
        let px = co.nx(col.max(0) as u32) * w;
        if let Some(top) = cliff_top(px)
            && (px - fx).abs() > fw * 1.4
            && top < ctx.horizon - 0.05
            && unit_from_hash(k) < 0.55
        {
            let shape = if k & 1 == 0 { BUSH } else { BUSH_SMALL };
            let row = layers::row_at(ctx, top) - 1;
            layers::stamp(ctx, cells, shape, col, row, 0.5);
            let y = co.ny(row.max(0) as u32) - 0.5 / rows;
            for c in (col - 2).max(0)..=(col + 2).min(i64::from(ctx.cols) - 1) {
                let s = &mut sky[c as usize];
                *s = s.min(y);
            }
        }
        col += 3 + (k >> 50) as i64 % 3;
    }

    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}
