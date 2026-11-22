//! Countryside: a patchwork of fields over gentle hills, hedgerows between
//! them, a windmill on a far rise, a barn and round trees; the focal group
//! stands on a grassy bank.

use super::layers::{self, Layer};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.14,
    amp: 0.05,
    freq: 1.3,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.28,
    outline: Some(0.42),
    snow: None,
    label: "country_hills",
};

/// Grass, light to heavy.
const GRASS: [u8; 4] = [id('.'), id(','), id('\''), id('"')];

/// The fields' hills: painted for their silhouette, then re-textured as
/// fields.
const FIELDS: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: -0.07,
    amp: 0.04,
    freq: 1.2,
    ridged: false,
    plateau: None,
    fill: (20.0, 20.0),
    threshold: 0.0,
    shade: 0.45,
    outline: Some(0.55),
    snow: None,
    label: "country_fields",
};

const NEAR: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: 0.11,
    amp: 0.03,
    freq: 2.2,
    ridged: false,
    plateau: None,
    fill: (16.0, 22.0),
    threshold: 0.42,
    shade: 0.6,
    outline: Some(0.7),
    snow: None,
    label: "country_near",
};

const WINDMILL: [&str; 11] = [
    "\\       /",
    " \\     / ",
    "  \\   /  ",
    "   \\ /   ",
    "    X    ",
    "   /#\\   ",
    "  /|#|\\  ",
    " / |#| \\ ",
    "/  |#|  \\",
    "   |#|   ",
    "  _|#|_  ",
];

const BARN: [&str; 6] = [
    "    _^_    ",
    "   /   \\   ",
    "  /_____\\  ",
    "  |  X  |  ",
    "  | |#| |  ",
    "  |_|#|_|  ",
];

const TREE: [&str; 4] = [" &&& ", "&&@&&", " &&& ", "  |  "];
const TREE_SMALL: [&str; 3] = [" & ", "&@&", " | "];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let seed = hash_label(ctx.terrain, "countryside");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0xFA7));
    let mut sky = layers::paint(ctx, cells, &[HILLS, FIELDS]);

    // Fields: bands across, warped, split again going down; each patch has
    // its own crop, and a hedgerow where two patches meet.
    let warp = hash_label(seed, "warp");
    let rows = f64::from(ctx.rows);
    let patch = |col: u32, row: u32| -> Option<(i64, i64)> {
        let nx = co.nx(col);
        let ny = co.ny(row);
        let top = FIELDS.top(ctx, nx);
        if ny < top + 1.0 / rows {
            return None;
        }
        let wx = nx * ctx.aspect * 2.4 + 0.35 * (fbm2(warp, nx * 6.0, ny * 6.0, 2) - 0.5);
        // Nearer fields look deeper: the rows per field grow going down.
        let dy = ((ny - top) * 26.0).sqrt() * 2.2 + 0.4 * (fbm2(warp ^ 1, nx * 4.0, 0.5, 2) - 0.5);
        Some((wx.floor() as i64, dy.floor() as i64))
    };
    let crop_seed = hash_label(seed, "crops");
    for row in 0..ctx.rows {
        for col in 0..ctx.cols {
            let Some((i, j)) = patch(col, row) else {
                continue;
            };
            let idx = ctx.idx(col, row);
            // Hedgerows: between bands across, and under some fields.
            let hedge = (col + 1 < ctx.cols && patch(col + 1, row).is_some_and(|p| p.0 != i))
                || (row + 1 < ctx.rows
                    && hash3(crop_seed ^ 0x4ED6, i, j).is_multiple_of(3)
                    && patch(col, row + 1).is_some_and(|p| p.1 != j));
            let k = hash3(crop_seed, i64::from(col), i64::from(row));
            let dim = if row > ctx.platform_row { 0.5 } else { 1.0 };
            if hedge {
                cells[idx] = Cell {
                    glyph: [id('&'), id('%'), id('o'), id('&')][(k >> 30) as usize % 4],
                    shade: shade_u8(0.44 * dim),
                };
                continue;
            }
            let kind = hash3(crop_seed, i, j) % 6;
            let (glyph, shade) = match kind {
                // Furrows.
                0 => (id('-'), 0.34),
                // Green crops in rows.
                1 => (
                    if (col + row) % 2 == 0 {
                        id('"')
                    } else {
                        id('\'')
                    },
                    0.42,
                ),
                // Wheat.
                2 => (
                    if k.is_multiple_of(3) {
                        id(',')
                    } else {
                        id(':')
                    },
                    0.48,
                ),
                // Pasture: grass here and there.
                3 => {
                    if unit_from_hash(k) < 0.45 {
                        (GRASS[(k >> 40) as usize % 2], 0.36)
                    } else {
                        cells[idx] = Cell::default();
                        continue;
                    }
                }
                // Ploughed earth.
                4 => (id('~'), 0.32),
                // Flowers.
                _ => {
                    if unit_from_hash(k) < 0.3 {
                        (id('*'), 0.55)
                    } else {
                        (id(','), 0.42)
                    }
                }
            };
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(shade * dim),
            };
        }
    }

    // The windmill on a far rise, the barn on the fields, both away from
    // the group.
    let group_left = ctx.group_cx < w * 0.5;
    let side = |a: f64, b: f64| if group_left { a } else { 1.0 - b };
    let shelter = |sky: &mut [f64], col: i64, half: i64, top_row: i64| {
        let y = co.ny(top_row.max(0) as u32) - 0.5 / rows;
        for c in (col - half).max(0)..=(col + half).min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
    };
    let mill_x = side(0.58 + 0.12 * pick(1), 0.58 + 0.12 * pick(1));
    let col = co.col_of(mill_x * w);
    if col > 5 && col < i64::from(ctx.cols) - 6 {
        let row = layers::row_at(ctx, HILLS.top(ctx, mill_x).min(FIELDS.top(ctx, mill_x))) + 1;
        layers::stamp(ctx, cells, &WINDMILL, col, row, 0.42);
        shelter(&mut sky, col, 5, row - WINDMILL.len() as i64);
    }
    let barn_x = side(0.82 + 0.08 * pick(2), 0.82 + 0.08 * pick(2));
    let col = co.col_of(barn_x * w);
    if (barn_x - ctx.group_cx / w).abs() > 0.14 && col > 6 && col < i64::from(ctx.cols) - 7 {
        let row = layers::row_at(ctx, FIELDS.top(ctx, barn_x)) + 2;
        layers::stamp(ctx, cells, &BARN, col, row, 0.5);
        shelter(&mut sky, col, 5, row - BARN.len() as i64);
    }
    // Round trees along the top of the fields.
    let tree_seed = hash_label(seed, "trees");
    let mut col = 2i64;
    while col < i64::from(ctx.cols) - 2 {
        let k = hash3(tree_seed, col, 3);
        let nx = co.nx(col as u32);
        let near_group = (nx * w - ctx.group_cx).abs() < w * 0.12;
        let near_mill = (nx - mill_x).abs() < 0.06 || (nx - barn_x).abs() < 0.07;
        if !near_group && !near_mill && unit_from_hash(k) < 0.3 {
            let shape: &[&str] = if k & 1 == 0 { &TREE } else { &TREE_SMALL };
            let row = layers::row_at(ctx, FIELDS.top(ctx, nx));
            layers::stamp(ctx, cells, shape, col, row, 0.46);
            shelter(&mut sky, col, 2, row - shape.len() as i64);
        }
        col += 5 + (k >> 50) as i64 % 6;
    }

    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}
