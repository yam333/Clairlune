//! A landmark: one big, far shape that stays put behind a journey's moving
//! layers (docs/scrolling-scenes.md). Only the land in front of it scrolls,
//! so the eye reads the depth: a granite dome, a lone peak, twin peaks, a
//! volcano, a mesa, or a moon rising huge behind the land. Composition picks
//! it (the recipe's `landmark`); recipes without one come out as before.

use super::{Ctx, shade_u8};
use crate::glyphs::id;
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, pow, smoothstep};
use crate::noise::{fbm1, fbm2};

/// Every landmark there is.
pub const ALL: [&str; 6] = ["dome", "peak", "twin_peaks", "volcano", "mesa", "moon_rise"];

/// The landmarks a journey through a landscape may have.
pub fn options(archetype: &str) -> &'static [&'static str] {
    match archetype {
        "mountains" => &["dome", "peak", "twin_peaks", "volcano"],
        "snowfield" => &["peak", "twin_peaks", "dome"],
        "forest" => &["dome", "peak", "volcano", "moon_rise"],
        "desert" => &["mesa", "volcano", "moon_rise", "dome"],
        _ => &[],
    }
}

fn pick(seed: u64, i: i64) -> f64 {
    unit_from_hash(hash3(seed, i, 0x1A4D))
}

/// One peak's height in 0..1 at `u` (-1..1 across its base), leaning by
/// `skew` (-0.3..0.3: the top off centre).
fn peak(u: f64, skew: f64) -> f64 {
    let u = if u < skew {
        (u - skew) / (1.0 + skew)
    } else {
        (u - skew) / (1.0 - skew)
    };
    if u.abs() >= 1.0 {
        0.0
    } else {
        pow(1.0 - u.abs(), 1.35)
    }
}

/// The silhouette's height in 0..1 at `u` (-1..1 across its base).
fn profile(kind: &str, u: f64, seed: u64) -> f64 {
    let a = u.abs();
    if a >= 1.0 {
        return 0.0;
    }
    match kind {
        // Steep sides, a broad rounded top.
        "dome" => pow(1.0 - u * u, 0.45),
        "peak" => peak(u, (pick(seed, 4) - 0.5) * 0.5),
        "twin_peaks" => {
            let left = peak((u + 0.42) / 0.58, 0.1);
            let right = peak((u - 0.40) / 0.60, -0.1) * (0.74 + 0.12 * pick(seed, 5));
            left.max(right)
        }
        // A broad cone cut flat at the crater, with a notch in its rim.
        "volcano" => {
            let cone = pow(1.0 - a, 1.15) / 0.86;
            let notch = 0.07 * (1.0 - smoothstep(0.0, 0.11, (u - 0.04).abs()));
            cone.min(1.0) - notch
        }
        // A flat-topped butte with sheer sides, a lower shoulder beside it.
        // A butte: a flat top, sheer cliffs, then a talus slope to the foot.
        "mesa" => {
            let cliff = 1.0 - 0.62 * smoothstep(0.36, 0.40, a);
            let talus = 0.38 * pow(clamp01((1.0 - a) / 0.6), 1.4);
            if a < 0.40 { cliff } else { talus.min(cliff) }
        }
        _ => 0.0,
    }
}

/// How wide it is for its height (half the base over the rise).
fn spread(kind: &str, seed: u64) -> f64 {
    let p = pick(seed, 2);
    match kind {
        "dome" => 1.25 + 0.35 * p,
        "peak" => 1.05 + 0.3 * p,
        "twin_peaks" => 1.6 + 0.3 * p,
        "volcano" => 1.7 + 0.4 * p,
        "mesa" => 1.1 + 0.3 * p,
        _ => 1.0,
    }
}

/// The texture inside: a few glyphs, light to heavy.
fn texture(kind: &str) -> &'static [u8] {
    const ROCK: [u8; 4] = [id('.'), id(':'), id('v'), id('^')];
    const DOME: [u8; 3] = [id('.'), id('v'), id('v')];
    const ASH: [u8; 4] = [id('.'), id(','), id(':'), id(';')];
    const STRATA: [u8; 4] = [id('-'), id('_'), id('='), id('=')];
    match kind {
        "dome" => &DOME,
        "volcano" => &ASH,
        "mesa" => &STRATA,
        _ => &ROCK,
    }
}

/// Paints the recipe's landmark into the still background and returns its
/// top (normalised y) per column, `INFINITY` where it doesn't reach. The
/// land painted after it hides its foot.
pub(crate) fn paint(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut top_of = vec![f64::INFINITY; ctx.cols as usize];
    let kind = ctx.recipe.composition.landmark.as_str();
    if kind.is_empty() {
        return top_of;
    }
    let seed = hash_label(ctx.terrain, "landmark");
    if kind == "moon_rise" {
        moon(ctx, cells, &mut top_of, seed);
        return top_of;
    }
    if !ALL.contains(&kind) {
        return top_of;
    }
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    // Far and tall: its top high in the sky, its foot behind the land.
    let base = ctx.horizon + 0.02;
    let top = 0.09 + 0.09 * pick(seed, 1);
    let rise = base - top;
    let half = (rise * h * spread(kind, seed)).min(w * 0.62);
    let cx = w * (0.28 + 0.44 * pick(seed, 3));
    let snowy = matches!(ctx.recipe.archetype.as_str(), "mountains" | "snowfield")
        && matches!(kind, "peak" | "twin_peaks");
    let rough = match kind {
        "mesa" => 0.004,
        "dome" => 0.008,
        _ => 0.016,
    };
    let edge_seed = hash_label(seed, "edge");
    let fill_seed = hash_label(seed, "fill");
    let height_at = |px: f64| -> f64 {
        let u = (px - cx) / half;
        let p = profile(kind, u, seed);
        if p <= 0.0 {
            return f64::INFINITY;
        }
        let n = fbm1(edge_seed, px / h * 9.0, 3) - 0.5;
        base - rise * p + n * rough * (0.4 + p)
    };
    let family = texture(kind);
    let rows_f = f64::from(ctx.rows);
    let cw = f64::from(co.g.cell_w);
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let t = height_at(px);
        if !t.is_finite() {
            continue;
        }
        top_of[col as usize] = t;
        // The face: rising to the right is a `/` slope, falling a `\`.
        let slope = (height_at(px + cw) - height_at(px - cw)) * h / (2.0 * cw);
        let slope = if slope.is_finite() { slope } else { 0.0 };
        let (face, stroke) = if slope.abs() > 3.0 {
            (id('|'), 0i64)
        } else if slope < -0.18 {
            (id('/'), 1)
        } else if slope > 0.18 {
            (id('\\'), -1)
        } else {
            (id('_'), 0)
        };
        let u = (px - cx) / half;
        let p = profile(kind, u, seed);
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < t {
                continue;
            }
            if ny > base + 0.12 {
                break;
            }
            let below = (ny - t) * rows_f;
            // Haze: fainter toward the foot.
            let haze = 1.0 - 0.45 * clamp01((ny - t) / rise.max(0.05));
            let idx = ctx.idx(col, row);
            if below < 1.0 {
                cells[idx] = Cell {
                    glyph: face,
                    shade: shade_u8(0.62 * haze),
                };
                continue;
            }
            if snowy && p > 0.72 && below < 1.0 + 7.0 * (p - 0.72) / 0.28 {
                let k = hash3(fill_seed, i64::from(col), i64::from(row));
                const SNOW: [u8; 3] = [id('*'), id('\''), id('.')];
                cells[idx] = Cell {
                    glyph: SNOW[(k % 3) as usize],
                    shade: shade_u8(0.66),
                };
                continue;
            }
            // Strokes along the face just under the edge, like light on it.
            let (c, r) = (i64::from(col), i64::from(row));
            if stroke != 0 && below < 4.0 && (c + stroke * r).rem_euclid(3) == 0 {
                cells[idx] = Cell {
                    glyph: face,
                    shade: shade_u8(0.5 * haze),
                };
                continue;
            }
            let n = fbm2(fill_seed, px / h * 7.0, ny * 30.0, 4);
            let d = clamp01((n - 0.5) * 1.9 + 0.5);
            if d < 0.42 {
                cells[idx] = Cell::default();
                continue;
            }
            let k = hash3(fill_seed, c, r);
            let jitter = (unit_from_hash(k) - 0.5) * 1.2;
            let i = ((d - 0.42) / 0.58 * family.len() as f64 + jitter)
                .floor()
                .clamp(0.0, family.len() as f64 - 1.0) as usize;
            cells[idx] = Cell {
                glyph: family[i],
                shade: shade_u8(0.44 * haze),
            };
        }
    }
    top_of
}

/// A moon far bigger than the sky's own, half risen behind the land: a pale
/// rim, dark seas and a scatter of craters.
fn moon(ctx: &Ctx, cells: &mut [Cell], top_of: &mut [f64], seed: u64) {
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    let r = h * (0.27 + 0.08 * pick(seed, 1));
    // On the far side from the group: a whole disc reads best.
    let cx = if ctx.group_cx < w * 0.5 {
        w * (0.62 + 0.14 * pick(seed, 3))
    } else {
        w * (0.24 + 0.14 * pick(seed, 3))
    };
    let cy = ctx.horizon * h - r * (0.25 + 0.2 * pick(seed, 6));
    let seas = hash_label(seed, "seas");
    let (cw, ch) = (f64::from(co.g.cell_w), f64::from(co.g.cell_h));
    for col in 0..ctx.cols {
        let x = co.nx(col) * w;
        let dx = (x - cx) / r;
        if dx.abs() >= 1.0 {
            continue;
        }
        let half = (1.0 - dx * dx).sqrt() * r;
        top_of[col as usize] = (cy - half) / h;
        for row in 0..ctx.rows {
            let y = co.ny(row) * h;
            let dy = (y - cy) / r;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist >= 1.0 {
                continue;
            }
            if y > ctx.horizon * h + 2.0 * ch {
                break;
            }
            let idx = ctx.idx(col, row);
            // The rim: a ring a cell or so wide.
            if dist > 1.0 - 1.2 * cw.max(ch) / r {
                cells[idx] = Cell {
                    glyph: id(':'),
                    shade: shade_u8(0.78),
                };
                continue;
            }
            let n = fbm2(seas, dx * 2.2 + 7.0, dy * 2.2, 4);
            let k = hash3(seas, i64::from(col), i64::from(row));
            let crater = unit_from_hash(k) < 0.025;
            let (glyph, shade) = if crater {
                (if k & 1 == 0 { id('o') } else { id('O') }, 0.6)
            } else if n < 0.44 {
                // The seas: darker, sparse.
                if unit_from_hash(k >> 8) < 0.5 {
                    (id('.'), 0.32)
                } else {
                    (crate::glyphs::EMPTY, 0.0)
                }
            } else {
                // Lighter toward the upper left, where the sun is.
                let lit = clamp01(0.6 - 0.25 * (dx + dy));
                (if n > 0.58 { id(':') } else { id('.') }, 0.46 + 0.26 * lit)
            };
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(shade),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_stay_in_range_and_end_at_the_base() {
        for kind in ALL.iter().filter(|k| **k != "moon_rise") {
            for i in -120..=120 {
                let u = f64::from(i) / 100.0;
                let p = profile(kind, u, 7);
                assert!((-0.1..=1.0001).contains(&p), "{kind} at {u}: {p}");
                if u.abs() >= 1.0 {
                    assert_eq!(p, 0.0, "{kind} past its base");
                }
            }
            let highest = (-100..=100)
                .map(|i| profile(kind, f64::from(i) / 100.0, 7))
                .fold(0.0, f64::max);
            assert!(highest > 0.8, "{kind} never gets tall: {highest}");
        }
    }

    #[test]
    fn every_option_is_a_landmark() {
        for a in ["mountains", "snowfield", "forest", "desert"] {
            assert!(!options(a).is_empty());
            for o in options(a) {
                assert!(ALL.contains(o), "{a}: {o}");
            }
        }
        assert!(options("cave").is_empty());
    }
}
