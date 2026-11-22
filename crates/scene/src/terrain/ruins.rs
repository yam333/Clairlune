//! Ruins: what is left of an old temple on a grassy rise, a colonnade of
//! standing and broken columns, a lintel or two still in place, an arch,
//! fallen blocks and vines; the focal group stands on a rock ledge.

use super::Ctx;
use super::layers::{self, Layer};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.1,
    amp: 0.06,
    freq: 1.2,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.28,
    outline: Some(0.42),
    snow: None,
    label: "ruins_hills",
};

/// Grass, light to heavy.
const GRASS: [u8; 4] = [id('.'), id(','), id('\''), id('"')];

/// The rise the ruins stand on: low and wide.
const RISE: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: -0.02,
    amp: 0.02,
    freq: 1.0,
    ridged: false,
    plateau: None,
    fill: (22.0, 24.0),
    threshold: 0.44,
    shade: 0.44,
    outline: Some(0.55),
    snow: None,
    label: "ruins_rise",
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
    label: "ruins_near",
};

const ARCH: [&str; 9] = [
    "  _______  ",
    " /=======\\ ",
    "|#/     \\#|",
    "|#|     |#|",
    "|#|     |#|",
    "|#|     |#|",
    "|#|     |#|",
    "|#|     |#|",
    "=#=     =#=",
];

/// Fallen blocks.
const RUBBLE: [&[&str]; 3] = [&["_", "#=#"], &["=#"], &["o#", "##="]];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "ruins");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x7E3));
    let mut sky = layers::paint(ctx, cells, &[HILLS, RISE]);
    let shelter = |sky: &mut [f64], c0: i64, c1: i64, top_row: i64| {
        let y = co.ny(top_row.max(0) as u32) - 0.5 / rows;
        for c in c0.max(0)..=c1.min(i64::from(ctx.cols) - 1) {
            let s = &mut sky[c as usize];
            *s = s.min(y);
        }
    };

    // The colonnade, on the far side from the group.
    let group_left = ctx.group_cx < w * 0.5;
    let span = 0.3 + 0.12 * pick(1);
    let (a, b) = if group_left {
        (0.95 - span, 0.95)
    } else {
        (0.05, 0.05 + span)
    };
    let gap = 6 + (pick(2) * 3.0) as i64;
    let c0 = co.col_of(a * w);
    let c1 = co.col_of(b * w);
    let tall = 12.0 + 6.0 * pick(3);
    let vine = hash_label(seed, "vines");
    let mut columns: Vec<(i64, i64, i64)> = Vec::new(); // col, base row, top row
    let mut col = c0;
    let mut i = 0i64;
    while col <= c1 {
        let k = hash3(seed, i, 0xC01);
        i += 1;
        if col < 2 || col > i64::from(ctx.cols) - 3 {
            col += gap;
            continue;
        }
        let nx = co.nx(col as u32);
        let base = layers::row_at(ctx, RISE.top(ctx, nx));
        // Some stand whole, some are broken off, a few are just a stump.
        let r = unit_from_hash(k);
        let height = if r < 0.5 {
            tall
        } else if r < 0.85 {
            tall * (0.4 + 0.4 * unit_from_hash(k.rotate_left(9)))
        } else {
            2.0
        } as i64;
        let whole = r < 0.5;
        let top = base - height;
        for row in top..=base {
            let glyph = if row == base {
                id('=')
            } else if row == top {
                if whole { id('=') } else { id('^') }
            } else {
                let kv = hash3(vine, col, row);
                if unit_from_hash(kv) < 0.12 {
                    [id('%'), id('&'), id(';')][(kv >> 30) as usize % 3]
                } else {
                    id('|')
                }
            };
            let shade = if glyph == id('|') { 0.55 } else { 0.5 };
            layers::put(ctx, cells, col, row, glyph, shade);
            // Fluting: a second stroke beside the first.
            if row != base && row != top {
                layers::put(ctx, cells, col + 1, row, id('|'), 0.42);
            }
        }
        if whole {
            layers::put(ctx, cells, col - 1, top, id('_'), 0.5);
            layers::put(ctx, cells, col + 2, top, id('_'), 0.5);
            layers::put(ctx, cells, col - 1, base, id('='), 0.5);
            layers::put(ctx, cells, col + 2, base, id('='), 0.5);
        }
        shelter(&mut sky, col - 1, col + 2, top);
        columns.push((col, base, if whole { top } else { i64::MIN }));
        col += gap;
    }
    // Lintels still resting on pairs of whole columns.
    for pair in columns.windows(2) {
        let (ca, _, ta) = pair[0];
        let (cb, _, tb) = pair[1];
        if ta != i64::MIN && tb != i64::MIN && ta == tb && !hash3(seed, ca, 0x11).is_multiple_of(3)
        {
            let row = ta - 1;
            for c in ca - 1..=cb + 2 {
                layers::put(ctx, cells, c, row, id('='), 0.55);
            }
            shelter(&mut sky, ca - 1, cb + 2, row);
        }
    }

    // An arch standing on its own, between the colonnade and the group.
    let arch_x = if group_left {
        a - 0.08 - 0.05 * pick(4)
    } else {
        b + 0.08 + 0.05 * pick(4)
    };
    let col = co.col_of(arch_x * w);
    if pick(5) < 0.7
        && (arch_x - ctx.group_cx / w).abs() > 0.12
        && col > 6
        && col < i64::from(ctx.cols) - 7
    {
        let row = layers::row_at(ctx, RISE.top(ctx, arch_x));
        layers::stamp(ctx, cells, &ARCH, col, row, 0.5);
        shelter(&mut sky, col - 5, col + 5, row - ARCH.len() as i64);
    }

    // Fallen blocks in the grass.
    let rubble = hash_label(seed, "rubble");
    let mut col = 3i64;
    while col < i64::from(ctx.cols) - 3 {
        let k = hash3(rubble, col, 5);
        let nx = co.nx(col as u32);
        if (nx * w - ctx.group_cx).abs() > w * 0.1 && unit_from_hash(k) < 0.3 {
            let shape = RUBBLE[(k >> 40) as usize % RUBBLE.len()];
            let row = layers::row_at(ctx, RISE.top(ctx, nx)) + 1 + (k >> 50) as i64 % 3;
            layers::stamp(ctx, cells, shape, col, row, 0.48);
        }
        col += 7 + (k >> 20) as i64 % 6;
    }

    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}
