//! Oasis: a pool among the dunes ringed with grass and date palms, a tent or
//! two in their shade with a lantern lit after dusk. The focal group stands
//! on a dune crest on this side.

use super::layers::{self, Layer, put};
use super::{Ctx, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

const FAR_DUNES: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.13,
    amp: 0.04,
    freq: 1.3,
    ridged: false,
    plateau: None,
    fill: (26.0, 16.0),
    threshold: 0.52,
    shade: 0.24,
    outline: Some(0.4),
    snow: None,
    label: "oasis_far",
};

const DUNES: Layer = Layer {
    family: &glyphs::SAND,
    hatch: None,
    base: -0.06,
    amp: 0.035,
    freq: 1.8,
    ridged: false,
    plateau: None,
    fill: (20.0, 26.0),
    threshold: 0.5,
    shade: 0.36,
    outline: Some(0.5),
    snow: None,
    label: "oasis_dunes",
};

/// A palm's crown, from its top.
const CROWN: &[&str] = &[
    "   _.  ._   ",
    " _-=\\  /=-_ ",
    "/  _-\\//-_  \\",
    "  / //%%\\\\ \\ ",
    " /  /    \\  \\",
];
const CROWN_SMALL: &[&str] = &["  _.  ._  ", "_-=\\//=-_", " / /%%\\ \\ ", "/  /  \\  \\"];
const TENT: &[&str] = &[
    "    _/\\_    ",
    "   /    \\   ",
    "  /      \\  ",
    " /   ##   \\ ",
    "/____##____\\",
];

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "oasis");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let mut sky = layers::paint(ctx, cells, &[FAR_DUNES, DUNES]);

    // The pool, on the far side: a flat oval of water.
    let far = if ctx.group_cx < w * 0.5 { 0.66 } else { 0.34 };
    let px = w * (far + 0.06 * (pick(0, 1) - 0.5));
    let pcol = co.col_of(px);
    let prow = layers::row_at(ctx, ctx.horizon - 0.035);
    let rx = (w * (0.18 + 0.04 * pick(0, 2)) / f64::from(co.g.cell_w)).round() as i64;
    let ry = ctx.scaled(5.5).max(3);
    let water = hash_label(seed, "water");
    for row in prow - ry - 1..=prow + ry + 1 {
        for c in pcol - rx - 3..=pcol + rx + 3 {
            if c < 0 || row < 0 || c >= i64::from(ctx.cols) || row >= i64::from(ctx.rows) {
                continue;
            }
            let u = (c - pcol) as f64 / rx as f64;
            let v = (row - prow) as f64 / ry as f64;
            let r = u * u + v * v;
            let idx = ctx.idx(c as u32, row as u32);
            if r < 1.0 {
                let n = fbm2(water, c as f64 * 0.2, row as f64 * 0.9, 2);
                cells[idx] = Cell {
                    glyph: if n > 0.56 {
                        id('~')
                    } else if n > 0.42 {
                        id('-')
                    } else {
                        EMPTY
                    },
                    shade: shade_u8(0.5),
                };
            } else if r < 1.35 {
                // A ring of grass round the water.
                let k = hash3(water, c, row);
                cells[idx] = Cell {
                    glyph: [id(','), id('"'), id('\''), id('v')][(k % 4) as usize],
                    shade: shade_u8(0.58),
                };
            }
        }
    }

    // Palms round the pool, leaning out over it.
    let palms = 5 + (pick(0, 3) * 4.0) as i64;
    let mut tops: Vec<(i64, i64)> = Vec::new();
    for p in 0..palms {
        let side = if p % 2 == 0 { -1.0 } else { 1.0 };
        let along = 0.55 + 0.5 * pick(p, 4);
        let c = pcol + (side * along * rx as f64) as i64;
        let root = prow + ((pick(p, 5) - 0.6) * ry as f64) as i64;
        let tall = ctx.scaled(14.0 + 12.0 * pick(p, 6)).max(6);
        let lean = -side * (0.15 + 0.2 * pick(p, 7));
        let mut tc = c;
        for k in 0..tall {
            let x = c + (lean * k as f64 * k as f64 / tall as f64) as i64;
            let glyph = if x == tc {
                if k % 3 == 0 { id('#') } else { id('|') }
            } else if (x - tc) * (side as i64) < 0 {
                id('/')
            } else {
                id('\\')
            };
            put(ctx, cells, x, root - k, glyph, 0.62);
            let bark = if k % 3 == 1 { id('|') } else { id(':') };
            put(ctx, cells, x + 1, root - k, bark, 0.5);
            tc = x;
        }
        let crown = if tall > ctx.scaled(20.0) {
            CROWN
        } else {
            CROWN_SMALL
        };
        layers::stamp(
            ctx,
            cells,
            crown,
            tc,
            root - tall + crown.len() as i64 - 1,
            0.66,
        );
        tops.push((tc, root - tall - 1));
    }
    for (c, top) in tops {
        let y = co.ny(top.max(0) as u32) - 0.5 / rows;
        for x in (c - 5).max(0)..=(c + 5).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, x, y);
        }
    }

    // A tent in the palms' shade, its lantern lit after dusk.
    let tcol = pcol + (rx as f64 * if far > 0.5 { 0.6 } else { -0.6 }) as i64;
    let trow = prow - ry;
    layers::stamp(ctx, cells, TENT, tcol, trow, 0.6);
    put(
        ctx,
        cells,
        tcol + 7,
        trow,
        id('o'),
        if lit(ctx.tod) { 1.0 } else { 0.5 },
    );
    sky
}
