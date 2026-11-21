//! Volcano: a great smoking cone over black lava fields, lava glowing down
//! its flanks; the focal group stands on a rock ledge.

use super::layers::{self, Layer};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, pow};
use crate::noise::{fbm1, fbm2};

const FIELDS: Layer = Layer {
    family: &glyphs::RIDGE,
    hatch: None,
    base: 0.02,
    amp: 0.05,
    freq: 2.2,
    ridged: true,
    plateau: None,
    fill: (14.0, 30.0),
    threshold: 0.45,
    shade: 0.5,
    outline: Some(0.6),
    snow: None,
    label: "volcano_fields",
};

/// The cone's slope texture, light to heavy.
const ROCK: [u8; 4] = [id('.'), id(':'), id('^'), id('x')];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    let seed = hash_label(ctx.terrain, "volcano");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x7011));
    // The cone, on the far side from the group.
    let cx = if ctx.group_cx < w * 0.5 {
        w * (0.6 + 0.2 * pick(1))
    } else {
        w * (0.2 + 0.2 * pick(1))
    };
    let base = ctx.horizon + 0.01;
    let top = 0.16 + 0.08 * pick(2);
    let half = (base - top) * h * (1.6 + 0.3 * pick(3));
    let crater = 0.1;
    let mut sky = vec![f64::INFINITY; ctx.cols as usize];
    let cone_top = |px: f64| -> Option<f64> {
        let u = (px - cx) / half;
        if u.abs() >= 1.0 {
            return None;
        }
        // Concave flanks up to a flat crater rim, a notch in the middle.
        let p = pow(1.0 - u.abs(), 1.25) / pow(1.0 - crater, 1.25);
        let p = p.min(1.0) - if u.abs() < crater * 0.6 { 0.03 } else { 0.0 };
        let n = (fbm1(seed, px / h * 10.0, 3) - 0.5) * 0.012;
        Some(base - (base - top) * p + n)
    };
    let lava_seed = hash_label(seed, "lava");
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let Some(t) = cone_top(px) else {
            continue;
        };
        sky[col as usize] = t;
        let u = (px - cx) / half;
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < t {
                continue;
            }
            if ny > base + 0.1 {
                break;
            }
            let idx = ctx.idx(col, row);
            let below = (ny - t) * f64::from(ctx.rows);
            if below < 1.0 {
                let glyph = if u < -0.12 {
                    id('/')
                } else if u > 0.12 {
                    id('\\')
                } else {
                    id('_')
                };
                cells[idx] = Cell {
                    glyph,
                    shade: shade_u8(0.6),
                };
                continue;
            }
            // Lava runs: a few winding lines from the rim down the flanks.
            let wind = fbm1(lava_seed, ny * 6.0, 2) - 0.5;
            let run = ((u + wind * 0.25) * 5.0).fract().abs();
            let depth = clamp01((ny - t) / (base - top));
            if u.abs() < 0.75 && run < 0.06 && depth < 0.85 {
                cells[idx] = Cell {
                    glyph: if depth < 0.4 { id('|') } else { id('~') },
                    shade: shade_u8(0.9),
                };
                continue;
            }
            let n = fbm2(seed, px / h * 8.0, ny * 26.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.5);
            cells[idx] = if d < 0.4 {
                Cell::default()
            } else {
                let i = (((d - 0.4) / 0.6) * ROCK.len() as f64) as usize;
                Cell {
                    glyph: ROCK[i.min(ROCK.len() - 1)],
                    shade: shade_u8(0.42 * (1.0 - 0.4 * depth)),
                }
            };
        }
    }
    // Smoke from the crater: a plume leaning with the wind, widening.
    let smoke = hash_label(seed, "smoke");
    let lean = if pick(4) < 0.5 { -1.0 } else { 1.0 };
    let crater_y = top;
    for row in 0..ctx.rows {
        let ny = co.ny(row);
        if ny >= crater_y {
            break;
        }
        let up = (crater_y - ny) / crater_y.max(0.05);
        let centre = cx + lean * up * up * w * 0.18;
        let spread = half * (0.05 + 0.35 * up);
        for col in 0..ctx.cols {
            let px = co.nx(col) * w;
            let off = (px - centre).abs() / spread;
            if off > 1.0 {
                continue;
            }
            let n = fbm2(smoke, px / h * 9.0, ny * 18.0, 3);
            if n < 0.45 + 0.3 * off {
                continue;
            }
            let idx = ctx.idx(col, row);
            const PUFF: [u8; 4] = [id('.'), id('o'), id('O'), id('@')];
            let i = (((n - 0.45) * 4.0) as usize).min(3);
            cells[idx] = Cell {
                glyph: PUFF[i],
                shade: shade_u8(0.32 * (1.0 - 0.5 * up)),
            };
            let s = &mut sky[col as usize];
            *s = s.min(ny);
        }
    }
    let fields = layers::paint(ctx, cells, &[FIELDS]);
    for (s, f) in sky.iter_mut().zip(fields) {
        *s = s.min(f);
    }
    sky
}
