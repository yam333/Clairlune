//! Giant mushrooms: a grove of towering toadstools with spotted domed caps
//! on pale stalks, small ones in the moss at their feet, spores drifting
//! in the air between them; the focal group stands on a mossy bank.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.12,
    amp: 0.05,
    freq: 1.4,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.26,
    outline: Some(0.4),
    snow: None,
    label: "mushroom_hills",
};

/// Moss, light to heavy.
const MOSS: [u8; 5] = [id('.'), id(','), id('\''), id('"'), id('&')];

const FLOOR: Layer = Layer {
    family: &MOSS,
    hatch: None,
    base: -0.02,
    amp: 0.025,
    freq: 1.8,
    ridged: false,
    plateau: None,
    fill: (22.0, 24.0),
    threshold: 0.46,
    shade: 0.42,
    outline: Some(0.52),
    snow: None,
    label: "mushroom_floor",
};

const NEAR: Layer = Layer {
    family: &MOSS,
    hatch: None,
    base: 0.11,
    amp: 0.03,
    freq: 2.4,
    ridged: false,
    plateau: None,
    fill: (16.0, 22.0),
    threshold: 0.42,
    shade: 0.58,
    outline: Some(0.68),
    snow: None,
    label: "mushroom_near",
};

/// The caps' skin, light to heavy.
const CAP: [u8; 4] = [id(':'), id('%'), id('&'), id('@')];

const SMALL: [&[&str]; 3] = [&["_o_", " | "], &["@", "|"], &[" _ ", "/O\\", " | "]];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let seed = hash_label(ctx.terrain, "mushrooms");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let mut sky = layers::paint(ctx, cells, &[HILLS, FLOOR]);

    // Three or four giants, the tallest farthest from the group, none over it.
    let group_left = ctx.group_cx < w * 0.5;
    let count = 3 + (pick(0, 1) * 2.0) as i64;
    let mut placed: Vec<(f64, f64)> = Vec::new();
    for i in 0..count {
        // Spread across the far side, and one may stand on the near side.
        let far = i < count - 1;
        let u = (f64::from(i as i32) + 0.3 + 0.4 * pick(i, 2)) / f64::from(count as i32);
        let mut nx = if far == group_left {
            0.52 + 0.45 * u
        } else {
            0.03 + 0.45 * u
        };
        if !far {
            nx = if group_left {
                0.03 + 0.1 * pick(i, 3)
            } else {
                0.87 + 0.1 * pick(i, 3)
            };
        }
        let half = 0.035 + 0.045 * pick(i, 4);
        if (nx * w - ctx.group_cx).abs() < (half + 0.08) * w
            || placed
                .iter()
                .any(|&(x, h)| (x - nx).abs() < (h + half) * 0.8)
        {
            continue;
        }
        placed.push((nx, half));
        let col = co.col_of(nx * w);
        let base = layers::row_at(ctx, FLOOR.top(ctx, nx));
        let stalk = ctx.scaled(14.0 + 16.0 * pick(i, 5)).max(4);
        let cap_w = ((half * w) / f64::from(co.g.cell_w)).round().max(4.0) as i64;
        // A dome: about two thirds as tall as it is wide, in pixels (cells
        // are taller than they are wide).
        let aspect = f64::from(co.g.cell_w) / f64::from(co.g.cell_h);
        let cap_h =
            ((cap_w as f64 * aspect * (0.6 + 0.3 * pick(i, 6))).round() as i64).clamp(3, 14);
        giant(
            ctx,
            cells,
            &mut sky,
            hash3(seed, i, 7),
            col,
            base,
            stalk,
            cap_h,
            cap_w,
        );
    }

    // Little ones in the moss, clear of the group.
    let small = hash_label(seed, "small");
    let mut col = 2i64;
    while col < i64::from(ctx.cols) - 2 {
        let k = hash3(small, col, 1);
        let nx = co.nx(col as u32);
        if (nx * w - ctx.group_cx).abs() > w * 0.1 && unit_from_hash(k) < 0.4 {
            let shape = SMALL[(k >> 40) as usize % SMALL.len()];
            let row = layers::row_at(ctx, FLOOR.top(ctx, nx)) + (k >> 52) as i64 % 3;
            layers::stamp(ctx, cells, shape, col, row, 0.5);
        }
        col += 4 + (k >> 20) as i64 % 5;
    }

    // Spores hanging in the air, brightest near the caps.
    let spores = hash_label(seed, "spores");
    for row in 0..ctx.platform_row {
        let ny = co.ny(row);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            if cells[idx].glyph != glyphs::EMPTY || ny < 0.08 {
                continue;
            }
            let k = hash3(spores, i64::from(col), i64::from(row));
            let near = placed
                .iter()
                .map(|&(x, h)| ((co.nx(col) - x).abs() / (h * 2.5)).min(1.0))
                .fold(1.0, f64::min);
            if unit_from_hash(k) < 0.004 + 0.02 * (1.0 - near) {
                let glyph = [id('.'), id('\''), id('*'), id('.')][(k >> 30) as usize % 4];
                put(
                    ctx,
                    cells,
                    i64::from(col),
                    i64::from(row),
                    glyph,
                    0.5 + 0.35 * (1.0 - near),
                );
            }
        }
    }

    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}

/// One giant mushroom: a stalk flaring at the foot with a ring part way up,
/// gills under a domed, spotted cap. Its top shelters the sky.
#[allow(clippy::too_many_arguments)]
fn giant(
    ctx: &Ctx,
    cells: &mut [Cell],
    sky: &mut [f64],
    seed: u64,
    col: i64,
    base: i64,
    stalk: i64,
    cap_h: i64,
    cap_w: i64,
) {
    let cap_bottom = base - stalk;
    let cap_top = cap_bottom - cap_h;
    // The stalk: two or three columns wide, wider at the foot.
    let half = (cap_w / 6).clamp(1, 3);
    for row in cap_bottom + 1..=base {
        let from_foot = base - row;
        let flare = if from_foot < 2 { 1 } else { 0 };
        let h = half + flare;
        for c in col - h..=col + h {
            let glyph = if row == base && flare > 0 && (c == col - h || c == col + h) {
                if c < col { id('/') } else { id('\\') }
            } else if c == col - h || c == col + h {
                id('|')
            } else if row == cap_bottom + stalk / 3 {
                id('=')
            } else {
                id(':')
            };
            put(ctx, cells, c, row, glyph, 0.5);
        }
    }
    // Gills: the cap's underside.
    for c in col - cap_w + 2..=col + cap_w - 2 {
        let glyph = if (c - col).abs() <= half {
            id('|')
        } else {
            id('\'')
        };
        put(ctx, cells, c, cap_bottom, glyph, 0.38);
    }
    // The cap: a dome, widest at its rim, with pale spots.
    let skin = hash_label(seed, "skin");
    for row in cap_top..cap_bottom {
        let t = (row - cap_top) as f64 / cap_h.max(1) as f64;
        let wide = (t * (2.0 - t)).sqrt();
        let reach = (cap_w as f64 * (0.25 + 0.75 * wide)).round() as i64;
        for c in col - reach..=col + reach {
            let edge = c == col - reach || c == col + reach;
            let glyph = if row == cap_top {
                id('_')
            } else if edge {
                if c < col { id('/') } else { id('\\') }
            } else {
                let k = hash3(skin, c, row);
                if unit_from_hash(k) < 0.14 {
                    // A spot.
                    if k & 1 == 0 { id('O') } else { id('o') }
                } else {
                    let n = fbm2(skin, c as f64 * 0.3, row as f64 * 0.5, 2);
                    family_glyph(&CAP, n, 0.0, skin, c.max(0) as u32, row.max(0) as u32)
                }
            };
            let spot = glyph == id('O') || glyph == id('o');
            put(ctx, cells, c, row, glyph, if spot { 0.82 } else { 0.55 });
        }
        // The rim curls under at the bottom row.
        if row == cap_bottom - 1 {
            put(ctx, cells, col - reach - 1, row + 1, id('_'), 0.5);
            put(ctx, cells, col + reach + 1, row + 1, id('_'), 0.5);
        }
    }
    let y = ctx.co.ny(cap_top.max(0) as u32) - 0.5 / f64::from(ctx.rows);
    for c in (col - cap_w - 1).max(0)..=(col + cap_w + 1).min(i64::from(ctx.cols) - 1) {
        let s = &mut sky[c as usize];
        *s = s.min(y);
    }
}
