//! Standing stones: an ancient ring of stones on a grassy rise across the
//! way, some paired under a lintel, runes on them glowing, a low altar in the
//! middle and mist lying in the grass; the focal group stands on a grassy
//! bank.

use super::Ctx;
use super::layers::{self, Layer, put};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{TAU, sin};
use crate::recipe::TimeOfDay;

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.1,
    amp: 0.05,
    freq: 1.2,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.26,
    outline: Some(0.4),
    snow: None,
    label: "stones_hills",
};

/// Grass, light to heavy.
const GRASS: [u8; 4] = [id('.'), id(','), id('\''), id('"')];

const RISE: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: -0.015,
    amp: 0.015,
    freq: 1.0,
    ridged: false,
    plateau: None,
    fill: (22.0, 24.0),
    threshold: 0.46,
    shade: 0.42,
    outline: Some(0.52),
    snow: None,
    label: "stones_rise",
};

const NEAR: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: 0.11,
    amp: 0.03,
    freq: 2.4,
    ridged: false,
    plateau: None,
    fill: (16.0, 22.0),
    threshold: 0.42,
    shade: 0.6,
    outline: Some(0.7),
    snow: None,
    label: "stones_near",
};

/// Runes carved into the stones.
const RUNES: [u8; 5] = [id('x'), id('+'), id('*'), id('<'), id('>')];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "standing_stones");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let mut sky = layers::paint(ctx, cells, &[HILLS, RISE]);
    let glow = if matches!(ctx.tod, TimeOfDay::DeepDusk | TimeOfDay::Night) {
        0.95
    } else {
        0.75
    };

    // The ring, seen from the side: an ellipse of stones on the far side,
    // the far ones smaller and higher, drawn back to front.
    let group_left = ctx.group_cx < w * 0.5;
    let cx = if group_left {
        w * (0.7 + 0.1 * pick(0, 1))
    } else {
        w * (0.2 + 0.1 * pick(0, 1))
    };
    let rx = w * (0.13 + 0.04 * pick(0, 2));
    let ground = RISE.top(ctx, cx / w);
    let ry = 0.06;
    let n = 9 + (pick(0, 3) * 4.0) as i64;
    let turn = pick(0, 4);
    let mut stones: Vec<(f64, i64, i64, i64, bool)> = (0..n)
        .filter(|&i| pick(i, 5) > 0.12) // a few have fallen
        .map(|i| {
            let a = TAU * (i as f64 / n as f64 + turn);
            let depth = sin(a); // -1 far, 1 near
            let x = cx + rx * sin(a + TAU / 4.0);
            let base = layers::row_at(ctx, ground + ry * depth);
            let h = ctx
                .scaled(4.0 + 2.5 * (depth + 1.0) + 5.0 * pick(i, 6))
                .max(3);
            (depth, co.col_of(x), base, h, depth > -0.3)
        })
        .collect();
    stones.sort_by(|a, b| a.0.total_cmp(&b.0));
    let runes = hash_label(seed, "runes");
    let mut tops: Vec<(i64, i64, f64)> = Vec::new();
    for (depth, col, base, h, wide) in &stones {
        // The altar goes in once the far half is drawn.
        if *depth > 0.0 && tops.iter().all(|t| t.2 <= 0.0) {
            altar(ctx, cells, co.col_of(cx), layers::row_at(ctx, ground) + 1);
        }
        let shade = 0.42 + 0.12 * (depth + 1.0) / 2.0;
        // Broad stones with rounded shoulders: five columns near, four far.
        let (l, r) = if *wide {
            (col - 2, col + 2)
        } else {
            (col - 1, col + 2)
        };
        let top = base - h;
        // Weathered tops: flat, worn to one side, or rounded.
        let worn = hash3(seed, *col, 0x70B) % 3;
        for row in top..=*base {
            for c in l..=r {
                let glyph = if row == top {
                    let skip = match worn {
                        0 => false,
                        1 => c == l || c == l + 1,
                        _ => c == l || c == r,
                    };
                    if skip {
                        continue;
                    }
                    id('_')
                } else if row == top + 1 && c == l && worn != 0 {
                    id('/')
                } else if row == top + 1 && c == r && worn == 2 {
                    id('\\')
                } else if c == l || c == r {
                    id('|')
                } else if (row + c) % 3 == 0 {
                    id('%')
                } else {
                    id('#')
                };
                put(ctx, cells, c, row, glyph, shade);
            }
        }
        // A rune about a third of the way down, glowing.
        let k = hash3(runes, *col, *base);
        if unit_from_hash(k) < 0.6 {
            let rc = if *wide { *col } else { col + (k & 1) as i64 };
            put(
                ctx,
                cells,
                rc,
                top + 1 + h / 3,
                RUNES[(k >> 30) as usize % RUNES.len()],
                glow,
            );
        }
        let y = co.ny((top - 1).max(0) as u32) - 0.5 / rows;
        for c in (l - 1).max(0)..=(r + 1).min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
        tops.push((*col, top, *depth));
    }
    // Lintels across pairs of neighbouring far stones of about one height.
    let mut far: Vec<&(i64, i64, f64)> = tops.iter().filter(|t| t.2 < -0.2).collect();
    far.sort_by_key(|t| t.0);
    for pair in far.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if (b.0 - a.0) <= 12 && (a.1 - b.1).abs() <= 2 && hash3(seed, a.0, 0x1A).is_multiple_of(2) {
            let row = a.1.min(b.1) - 1;
            for c in a.0 - 2..=b.0 + 2 {
                put(ctx, cells, c, row, id('='), 0.5);
            }
        }
    }

    // Mist lying in the grass around the ring.
    let mist = hash_label(seed, "mist");
    let mist_row = layers::row_at(ctx, ground + 0.02);
    for row in mist_row..mist_row + 3 {
        for col in 0..ctx.cols {
            let x = co.nx(col) * w;
            if (x - cx).abs() > rx * 1.8 || (x - ctx.group_cx).abs() < w * 0.08 {
                continue;
            }
            let k = hash3(mist, i64::from(col), row);
            if unit_from_hash(k) < 0.3 && row >= 0 && row < i64::from(ctx.rows) {
                let idx = ctx.idx(col, row as u32);
                if cells[idx].glyph == EMPTY || unit_from_hash(k.rotate_left(9)) < 0.4 {
                    put(ctx, cells, i64::from(col), row, id('~'), 0.32);
                }
            }
        }
    }

    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}

/// A low flat stone in the middle of the ring.
fn altar(ctx: &Ctx, cells: &mut [Cell], col: i64, row: i64) {
    for c in col - 3..=col + 3 {
        put(ctx, cells, c, row - 1, id('='), 0.5);
        put(
            ctx,
            cells,
            c,
            row,
            if c == col - 3 || c == col + 3 {
                id('|')
            } else {
                id('#')
            },
            0.45,
        );
    }
}
