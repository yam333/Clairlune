//! Night sky: the sky is the subject. A moon (phase and craters from the seed)
//! on the side away from the focal group, aurora curtains after dark, a band of
//! extra stars, small islets drifting far off and a sea of cloud far below;
//! the focal group stands on a floating island trailing roots.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, pm, smoothstep};
use crate::noise::{fbm1, fbm2};
use crate::recipe::TimeOfDay;

const CLOUD: [u8; 4] = [id('.'), id('-'), id('~'), id('=')];
const MOON: [u8; 5] = [id(':'), id('o'), id('O'), id('%'), id('@')];
const AURORA: [u8; 4] = [id('\''), id(':'), id('|'), id('!')];

const CLOUDS: Layer = Layer {
    family: &CLOUD,
    hatch: None,
    base: 0.24,
    amp: 0.03,
    freq: 1.6,
    ridged: false,
    plateau: None,
    fill: (8.0, 40.0),
    threshold: 0.55,
    shade: 0.3,
    outline: Some(0.38),
    snow: None,
    label: "night_clouds",
};

fn pick(seed: u64, salt: i64) -> f64 {
    unit_from_hash(hash3(seed, salt, 0x51C))
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let seed = hash_label(ctx.terrain, "night_sky");
    let dark = matches!(ctx.tod, TimeOfDay::Night | TimeOfDay::DeepDusk);
    milky_way(ctx, cells, seed);
    if dark {
        aurora(ctx, cells, seed);
    }
    moon(ctx, cells, seed);
    for i in 0..3 {
        islet(ctx, cells, seed, i);
    }
    layers::paint(ctx, cells, &[CLOUDS])
}

/// A diagonal band of faint extra stars.
fn milky_way(ctx: &Ctx, cells: &mut [Cell], seed: u64) {
    let tilt = (pick(seed, 1) - 0.5) * 0.8;
    let y0 = 0.15 + 0.25 * pick(seed, 2);
    for row in 0..ctx.platform_row {
        for col in 0..ctx.cols {
            let (nx, ny) = (ctx.co.nx(col), ctx.co.ny(row));
            let centre = y0 + tilt * (nx - 0.5) + 0.04 * (fbm1(seed, nx * 3.0, 2) - 0.5);
            let d = ((ny - centre) / 0.07).abs();
            if d > 1.0 {
                continue;
            }
            let u = unit_from_hash(hash3(seed ^ 0x3A1, i64::from(col), i64::from(row)));
            if u < 0.22 * (1.0 - d * d) {
                let glyph = glyphs::STARS[(u * 97.0) as usize % 4];
                put(
                    ctx,
                    cells,
                    i64::from(col),
                    i64::from(row),
                    glyph,
                    0.18 + 0.2 * u,
                );
            }
        }
    }
}

/// Curtains of light: a sharp, wavy lower edge with rays rising from it and
/// fading upward, brightest where the curtain folds toward the viewer.
fn aurora(ctx: &Ctx, cells: &mut [Cell], seed: u64) {
    let bands = 1 + (pick(seed, 10) * 2.0) as i64;
    for b in 0..bands {
        let s = hash3(seed, b, 11);
        let y = 0.16 + 0.14 * pick(s, 1);
        let length = 0.10 + 0.08 * pick(s, 2);
        for col in 0..ctx.cols {
            let nx = ctx.co.nx(col);
            let strength = smoothstep(0.42, 0.7, fbm1(s, nx * 2.0, 3));
            if strength <= 0.0 {
                continue;
            }
            // The lower edge folds back and forth; rays vary column to column.
            let edge = y + 0.10 * (fbm1(s ^ 7, nx * 2.4, 3) - 0.5);
            let ray = length * (0.35 + 1.3 * fbm1(s ^ 9, nx * 22.0, 2));
            for row in 0..ctx.platform_row {
                let ny = ctx.co.ny(row);
                if ny > edge || ny < edge - ray {
                    continue;
                }
                let t = (edge - ny) / ray; // 0 at the lower edge, 1 at the ray tip
                let flicker = fbm2(s ^ 13, nx * 30.0, ny * 4.0, 2);
                let d = strength * (1.0 - t).powf(1.4) * (0.7 + 0.6 * flicker);
                if d < 0.2 {
                    continue;
                }
                let glyph = family_glyph(&AURORA, clamp01(d), 0.2, s, col, row);
                put(
                    ctx,
                    cells,
                    i64::from(col),
                    i64::from(row),
                    glyph,
                    0.18 + 0.4 * d,
                );
            }
        }
    }
}

/// The moon on the side away from the focal group: a disc of heavy glyphs with
/// darker craters, cut to a crescent or gibbous phase by a second disc.
fn moon(ctx: &Ctx, cells: &mut [Cell], seed: u64) {
    let g = &ctx.co.g;
    let group_left = ctx.group_cx < ctx.co.w * 0.5;
    let side = 0.62 + 0.22 * pick(seed, 20);
    let cx_px = ctx.co.w * if group_left { side } else { 1.0 - side };
    let cy_px = ctx.co.h * (0.12 + 0.12 * pick(seed, 21));
    let r_px = ctx.unit * (0.05 + 0.03 * pick(seed, 22));
    // Phase: the shadow disc's offset in radii (2+ is a full moon).
    let phase = [0.6, 0.9, 1.3, 2.5][(pick(seed, 23) * 4.0) as usize];
    let dir = if pick(seed, 24) < 0.5 { -1.0 } else { 1.0 };
    let (cw, ch) = (f64::from(g.cell_w), f64::from(g.cell_h));
    let c_lo = ((cx_px - r_px - f64::from(g.origin_x)) / cw).floor() as i64;
    let c_hi = ((cx_px + r_px - f64::from(g.origin_x)) / cw).ceil() as i64;
    let r_lo = ((cy_px - r_px - f64::from(g.origin_y)) / ch).floor() as i64;
    let r_hi = ((cy_px + r_px - f64::from(g.origin_y)) / ch).ceil() as i64;
    for r in r_lo..=r_hi {
        for c in c_lo..=c_hi {
            let px = f64::from(g.origin_x) + (c as f64 + 0.5) * cw;
            let py = f64::from(g.origin_y) + (r as f64 + 0.5) * ch;
            let (u, v) = ((px - cx_px) / r_px, (py - cy_px) / r_px);
            if u * u + v * v > 1.0 {
                continue;
            }
            let (su, sv) = (u - dir * phase, v);
            if su * su + sv * sv < 1.0 {
                put(ctx, cells, c, r, EMPTY, 0.0); // the unlit part hides stars
                continue;
            }
            let crater = fbm2(seed ^ 0xC4A7, u * 2.2 + 5.0, v * 2.2, 3);
            let d = clamp01(0.95 - smoothstep(0.55, 0.7, crater) * 0.55 - 0.15 * (u * u + v * v));
            let glyph = family_glyph(&MOON, d, 0.0, seed, c.max(0) as u32, r.max(0) as u32);
            put(ctx, cells, c, r, glyph, 0.55 + 0.35 * d);
        }
    }
}

/// A small floating rock far off: a flat top tapering to a point.
fn islet(ctx: &Ctx, cells: &mut [Cell], seed: u64, i: i64) {
    let s = hash3(seed, i, 30);
    let nx = 0.08 + 0.84 * pick(s, 1);
    let col = ctx.co.col_of(nx * ctx.co.w);
    let half = 2 + (pick(s, 2) * 4.0) as i64;
    let px0 = ctx.co.g.origin_x as f64 + (col - half - 2) as f64 * f64::from(ctx.co.g.cell_w);
    let px1 = ctx.co.g.origin_x as f64 + (col + half + 2) as f64 * f64::from(ctx.co.g.cell_w);
    if px1 > f64::from(ctx.gx0) && px0 < f64::from(ctx.gx1) {
        return; // never behind the focal group
    }
    let ny = ctx.horizon - 0.05 - 0.25 * pick(s, 3);
    let row = ((ny * ctx.co.h - f64::from(ctx.co.g.origin_y)) / f64::from(ctx.co.g.cell_h)) as i64;
    let shade = 0.3 + 0.12 * pick(s, 4);
    for k in 0..=half {
        let w = half - k;
        for c in col - w..=col + w {
            let glyph = if k == 0 {
                id('_')
            } else {
                family_glyph(&glyphs::NEAR, 0.4 + 0.5 * pick(s, c * 7 + k), 0.0, s, 0, 0)
            };
            put(ctx, cells, c, row + k, glyph, shade);
        }
    }
}

/// The floating island under the focal group: a grassy top spanning the group,
/// a rock body narrowing to a point, and roots hanging below.
pub(crate) fn island(ctx: &Ctx, cells: &mut [Cell]) {
    let co = &ctx.co;
    let comp = &ctx.recipe.composition;
    let margin_px = pm(comp.platform_margin_pm) * ctx.unit + 2.0 * f64::from(co.g.cell_w);
    let c0 = co.col_of(f64::from(ctx.gx0) - margin_px) as f64;
    let c1 = co.col_of(f64::from(ctx.gx1) + margin_px) as f64;
    let seed = hash_label(ctx.terrain, "island");
    let top = i64::from(ctx.platform_row);
    let half = (c1 - c0) * 0.5;
    let mid = (c0 + c1) * 0.5;
    // Depth in rows: about a third of the width in pixels.
    let depth = (half * f64::from(co.g.cell_w) * 0.9 / f64::from(co.g.cell_h)).max(4.0);
    let mut tips = Vec::new();
    for k in 0..=depth.ceil() as i64 {
        let t = k as f64 / depth;
        let w = half * (1.0 - t.powf(1.6)) + (fbm1(seed, k as f64 * 0.4, 2) - 0.5) * 3.0 * t;
        if w < 0.5 {
            break;
        }
        let (l, r) = ((mid - w).round() as i64, (mid + w).round() as i64);
        put(ctx, cells, l - 1, top + k, EMPTY, 0.0);
        put(ctx, cells, r + 1, top + k, EMPTY, 0.0);
        for c in l..=r {
            let glyph = if k == 0 {
                id('"')
            } else {
                let n = fbm2(seed, c as f64 * 0.3, k as f64 * 0.5, 3);
                family_glyph(
                    &glyphs::PLATFORM,
                    clamp01((n - 0.5) * 1.8 + 0.45),
                    0.0,
                    seed,
                    c.max(0) as u32,
                    k as u32,
                )
            };
            let shade = if k == 0 { 0.85 } else { 0.75 - 0.3 * t };
            put(ctx, cells, c, top + k, glyph, shade);
        }
        tips.push((l, r, top + k));
    }
    // Roots from the underside: short hanging strands.
    for (i, &(l, r, row)) in tips.iter().enumerate().skip(1) {
        for c in [l, r] {
            if unit_from_hash(hash3(seed, c, i as i64)) < 0.35 {
                let len = 1 + (unit_from_hash(hash3(seed, c, 99)) * 3.0) as i64;
                for d in 1..=len {
                    let glyph = if d == len { id('\'') } else { id('|') };
                    put(ctx, cells, c, row + d, glyph, 0.45);
                }
            }
        }
    }
    if let Some(&(l, r, row)) = tips.last() {
        let c = (l + r) / 2;
        put(ctx, cells, c, row + 1, id('v'), 0.5);
        put(ctx, cells, c, row + 2, id('\''), 0.35);
    }
}
