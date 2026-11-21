//! The Moon: grey crater fields under a black sky, the Earth hanging over
//! the horizon; the focal group stands on a crater rim.

use super::layers::{self, Layer};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;

const DUST: [u8; 4] = [id('.'), id(':'), id('o'), id('O')];

const FAR: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.06,
    amp: 0.05,
    freq: 1.4,
    ridged: true,
    plateau: None,
    fill: (26.0, 16.0),
    threshold: 0.5,
    shade: 0.3,
    outline: Some(0.5),
    snow: None,
    label: "lunar_far",
};
const PLAIN: Layer = Layer {
    family: &DUST,
    hatch: None,
    base: 0.05,
    amp: 0.025,
    freq: 2.0,
    ridged: false,
    plateau: None,
    fill: (14.0, 22.0),
    threshold: 0.55,
    shade: 0.5,
    outline: Some(0.6),
    snow: None,
    label: "lunar_plain",
};

const CRATER: [&str; 2] = [" .--. ", "<____>"];
const CRATER_SMALL: [&str; 2] = [".-.", "<_>"];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = vec![f64::INFINITY; ctx.cols as usize];
    earth(ctx, cells, &mut sky);
    let land = layers::paint(ctx, cells, &[FAR, PLAIN]);
    for (s, l) in sky.iter_mut().zip(land) {
        *s = s.min(l);
    }
    // Craters over the plain, clear of the group.
    let seed = hash_label(ctx.terrain, "lunar_craters");
    let g0 = ctx.co.col_of(f64::from(ctx.gx0)) - 6;
    let g1 = ctx.co.col_of(f64::from(ctx.gx1)) + 6;
    for i in 0..(10.0 * ctx.aspect) as i64 {
        let k = hash3(seed, i, 0);
        let col = (unit_from_hash(k) * f64::from(ctx.cols)) as i64;
        if (g0..=g1).contains(&col) {
            continue;
        }
        let top = PLAIN.top(ctx, ctx.co.nx(col.max(0) as u32));
        let below = unit_from_hash(k >> 9) * (1.0 - top);
        let row = layers::row_at(ctx, top + 0.03 + below * 0.8);
        let shape: &[&str] = if k & 3 == 0 { &CRATER } else { &CRATER_SMALL };
        layers::stamp(ctx, cells, shape, col, row, 0.6);
    }
    sky
}

/// The Earth: a disc a little above the horizon, oceans and continents,
/// lit from one side.
fn earth(ctx: &Ctx, cells: &mut [Cell], sky: &mut [f64]) {
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    let seed = hash_label(ctx.terrain, "lunar_earth");
    let p = |i: i64| unit_from_hash(hash3(seed, i, 0xEA57));
    let r = h * (0.07 + 0.03 * p(1));
    let cx = if ctx.group_cx < w * 0.5 {
        w * (0.62 + 0.2 * p(2))
    } else {
        w * (0.18 + 0.2 * p(2))
    };
    let cy = h * (0.18 + 0.12 * p(3));
    let lit = if p(4) < 0.5 { -1.0 } else { 1.0 };
    for col in 0..ctx.cols {
        let x = co.nx(col) * w;
        let dx = (x - cx) / r;
        if dx.abs() >= 1.0 {
            continue;
        }
        let half = (1.0 - dx * dx).sqrt();
        sky[col as usize] = sky[col as usize].min((cy - half * r) / h);
        for row in 0..ctx.rows {
            let y = co.ny(row) * h;
            let dy = (y - cy) / r;
            if dx * dx + dy * dy >= 1.0 {
                continue;
            }
            let idx = ctx.idx(col, row);
            // The night side shows only its rim.
            let day = dx * lit + 0.25;
            if day < 0.0 {
                if dx * dx + dy * dy > 0.82 {
                    cells[idx] = Cell {
                        glyph: id('.'),
                        shade: shade_u8(0.25),
                    };
                }
                continue;
            }
            let n = fbm2(seed, dx * 2.5 + 3.0, dy * 2.5, 4);
            let cloud = fbm2(seed ^ 0xC10D, dx * 4.0, dy * 7.0, 3);
            let (glyph, shade) = if cloud > 0.66 {
                (id('~'), 0.75)
            } else if n > 0.55 {
                (if n > 0.63 { id('%') } else { id('&') }, 0.6)
            } else {
                (id('.'), 0.45)
            };
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(shade * (0.6 + 0.4 * day.min(1.0))),
            };
        }
    }
}
