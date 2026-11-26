//! Observatory: a peak across the valley with domed observatories on its
//! summit, one dome open with its telescope out; the slit glows after dusk
//! and the Milky Way crosses the sky above. The focal group stands on a rock
//! ledge on this side.

use super::layers::{self, Layer, Snow, put};
use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, smoothstep};
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

const SNOWCAP: [u8; 3] = [id('.'), id(':'), id('\'')];

const RANGE: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: Some((&glyphs::FAR_LEFT, &glyphs::FAR_RIGHT)),
    base: -0.13,
    amp: 0.07,
    freq: 1.5,
    ridged: true,
    plateau: None,
    fill: (26.0, 14.0),
    threshold: 0.5,
    shade: 0.26,
    outline: Some(0.42),
    snow: Some(Snow {
        line: 0.3,
        rows: 2.0,
        glyphs: &SNOWCAP,
        shade: 0.5,
    }),
    label: "observatory_range",
};

const SLOPES: Layer = Layer {
    family: &glyphs::RIDGE,
    hatch: None,
    base: 0.015,
    amp: 0.025,
    freq: 2.2,
    ridged: true,
    plateau: None,
    fill: (16.0, 26.0),
    threshold: 0.48,
    shade: 0.42,
    outline: Some(0.52),
    snow: None,
    label: "observatory_slopes",
};

/// The peak's rock, light to heavy.
const ROCK: [u8; 4] = [id('.'), id(':'), id('x'), id('%')];

const HOUSE: &[&str] = &["_______", "|#o#o#|", "|##|##|"];

fn dark(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "observatory");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x0B5));
    let night = dark(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[RANGE]);

    // The peak: a broad cone on the far side with a flattened summit.
    let far = if ctx.group_cx < w * 0.5 { 0.7 } else { 0.3 };
    let cx = w * (far + 0.08 * (pick(1) - 0.5));
    let half = w * (0.2 + 0.05 * pick(2));
    let summit = ctx.horizon - (0.2 + 0.05 * pick(3));
    let flat = 0.16;
    let rock = hash_label(seed, "peak");
    let top_of = |px: f64| -> Option<f64> {
        let u = ((px - cx) / half).abs();
        if u >= 1.0 {
            return None;
        }
        let rise = 1.0 - smoothstep(flat, 1.0, u);
        Some(ctx.horizon + 0.02 - (ctx.horizon + 0.02 - summit) * rise)
    };
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let Some(t) = top_of(px) else {
            continue;
        };
        sky[col as usize] = sky[col as usize].min(t);
        let u = (px - cx) / half;
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < t {
                continue;
            }
            if ny > ctx.horizon + 0.04 {
                break;
            }
            let idx = ctx.idx(col, row);
            if (ny - t) * rows < 1.0 {
                cells[idx] = Cell {
                    glyph: if u.abs() < flat {
                        id('_')
                    } else if u < 0.0 {
                        id('/')
                    } else {
                        id('\\')
                    },
                    shade: shade_u8(0.55),
                };
                continue;
            }
            let n = fbm2(rock, px / co.h * 14.0, ny * 22.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.5);
            cells[idx] = if d < 0.4 {
                Cell::default()
            } else {
                Cell {
                    glyph: family_glyph(&ROCK, d, 0.4, rock, col, row),
                    shade: shade_u8(0.4),
                }
            };
        }
    }

    // The domes on the summit: a big one, open, and a small one.
    let base = layers::row_at(ctx, summit);
    let ccol = co.col_of(cx);
    let big = ctx.scaled(7.0).max(4);
    let small = ctx.scaled(4.0).max(3);
    let spread = co.col_of(cx + half * flat * 0.6) - ccol;
    let big_col = ccol - spread / 2;
    let small_col = ccol + spread;
    let top_big = dome(ctx, cells, big_col, base - 3, big, night, true);
    let top_small = dome(ctx, cells, small_col, base - 1, small, night, false);
    layers::stamp(ctx, cells, HOUSE, big_col, base, 0.5);
    if night {
        put(ctx, cells, big_col - 2, base - 1, id('o'), 0.95);
        put(ctx, cells, big_col + 2, base - 1, id('o'), 0.95);
    }
    for (c, top, r) in [(big_col, top_big, big), (small_col, top_small, small)] {
        let y = co.ny(top.max(0) as u32) - 0.5 / rows;
        for x in (c - 2 * r - 6).max(0)..=(c + 2 * r + 6).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, x, y);
        }
    }

    // The Milky Way: a slanting band of faint stars, after dusk.
    if night {
        let band = hash_label(seed, "milky_way");
        let tilt = 0.35 + 0.3 * pick(4);
        let at = 0.1 + 0.2 * pick(5);
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            for col in 0..ctx.cols {
                let nx = co.nx(col);
                if ny >= sky[col as usize] - 0.02 {
                    continue;
                }
                let off = (ny - (at + tilt * (nx - 0.5))).abs();
                let width = 0.07 + 0.03 * fbm2(band, nx * 4.0, 0.0, 2);
                if off > width {
                    continue;
                }
                let k = hash3(band, i64::from(col), i64::from(row));
                let dens = 0.35 * (1.0 - off / width);
                let idx = ctx.idx(col, row);
                if cells[idx].glyph == EMPTY && unit_from_hash(k) < dens {
                    let glyph =
                        [id('.'), id('`'), id('\''), id('.'), id('*')][(k >> 33) as usize % 5];
                    cells[idx] = Cell {
                        glyph,
                        shade: shade_u8(0.3 + 0.35 * unit_from_hash(k.rotate_left(13))),
                    };
                }
            }
        }
    }

    let slopes = layers::paint(ctx, cells, &[SLOPES]);
    for (s, n) in sky.iter_mut().zip(slopes) {
        *s = s.min(n);
    }
    sky
}

/// A dome `r` rows high on a drum, centred on `col` with its foot on `row`
/// (cells are about twice as tall as wide, so it is `4 r` columns across).
/// Open: a slit with the telescope out of it. Returns its top row.
fn dome(ctx: &Ctx, cells: &mut [Cell], col: i64, row: i64, r: i64, night: bool, open: bool) -> i64 {
    // The drum.
    for k in 0..2 {
        let half = 2 * r;
        put(ctx, cells, col - half, row - k, id('|'), 0.55);
        put(ctx, cells, col + half, row - k, id('|'), 0.55);
        for c in col - half + 1..col + half {
            put(ctx, cells, c, row - k, id('='), 0.45);
        }
    }
    let foot = row - 2;
    for k in 0..r {
        let y = k as f64 / r as f64;
        let half = (2.0 * r as f64 * (1.0 - y * y).sqrt()).round() as i64;
        let rr = foot - k;
        put(ctx, cells, col - half, rr, id('/'), 0.6);
        put(ctx, cells, col + half, rr, id('\\'), 0.6);
        for c in col - half + 1..col + half {
            let slit = open && (c == col + 1 || c == col + 2);
            let glyph = if slit {
                if night { id(':') } else { id('|') }
            } else if (c + k) % 4 == 0 {
                id('#')
            } else {
                id('=')
            };
            put(
                ctx,
                cells,
                c,
                rr,
                glyph,
                if slit && night { 0.9 } else { 0.5 },
            );
        }
    }
    let top = foot - r;
    for c in col - 2..=col + 2 {
        put(ctx, cells, c, top, id('_'), 0.6);
    }
    if open {
        // The telescope, pointing out of the slit.
        for k in 0..r {
            put(ctx, cells, col + 2 + k, top - 1 - k / 2, id('/'), 0.6);
        }
    }
    top - r / 2 - 1
}
