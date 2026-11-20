//! Mountains: three ranges of ridged peaks, snow on the high ones, hatching
//! that follows each face; the focal group stands on a rock ledge in front.

use super::layers::{self, Layer, Snow, Strip, StripCells};
use super::{Ctx, Journeyed};
use crate::glyphs::{self, id};
use crate::graph::{Cell, ScrollLayer};
use crate::hash::{hash_label, hash3, unit_from_hash};

const SNOWCAP: [u8; 3] = [id('.'), id(':'), id('\'')];

/// Hatching along a face: strokes parallel to the slope.
const FACE_RISE: [u8; 4] = [id('\''), id('.'), id('-'), id('/')];
const FACE_FALL: [u8; 4] = [id('\''), id('.'), id('-'), id('\\')];

const RANGES: [Layer; 3] = [
    Layer {
        family: &glyphs::DISTANT,
        hatch: None,
        base: -0.30,
        amp: 0.12,
        freq: 1.1,
        ridged: true,
        plateau: None,
        fill: (30.0, 14.0),
        threshold: 0.45,
        shade: 0.35,
        outline: Some(0.55),
        snow: Some(Snow {
            line: 0.2,
            rows: 2.0,
            glyphs: &SNOWCAP,
            shade: 0.6,
        }),
        label: "mountains_far",
    },
    Layer {
        family: &glyphs::RIDGE,
        hatch: Some((&FACE_RISE, &FACE_FALL)),
        base: -0.16,
        amp: 0.09,
        freq: 1.9,
        ridged: true,
        plateau: None,
        fill: (24.0, 12.0),
        threshold: 0.46,
        shade: 0.5,
        outline: Some(0.7),
        snow: Some(Snow {
            line: 0.5,
            rows: 1.5,
            glyphs: &SNOWCAP,
            shade: 0.7,
        }),
        label: "mountains_mid",
    },
    Layer {
        family: &glyphs::NEAR,
        hatch: None,
        base: 0.05,
        amp: 0.05,
        freq: 3.0,
        ridged: true,
        plateau: None,
        fill: (18.0, 14.0),
        threshold: 0.55,
        shade: 0.5,
        outline: Some(0.85),
        snow: None,
        label: "mountains_near",
    },
];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    layers::paint(ctx, cells, &RANGES)
}

/// The ridge a journey walks along: rolling rather than jagged, so the way
/// reads as walkable.
const RIDGE: Layer = Layer {
    family: &glyphs::NEAR,
    hatch: None,
    base: 0.08,
    amp: 0.075,
    freq: 1.7,
    ridged: false,
    plateau: None,
    fill: (18.0, 14.0),
    threshold: 0.55,
    shade: 0.5,
    outline: Some(0.85),
    snow: None,
    label: "mountains_ridge",
};

/// Cairns, rocks and the odd signpost along the way.
const WAYMARKS: [&[&str]; 4] = [&["o"], &["oO"], &[" o", "oOo"], &["-=>", " | "]];

/// A journey along the range (docs/scrolling-scenes.md): the far peaks are
/// far enough to stay put, so they go with the sky; the middle range slides
/// by one screen per loop and the ridge underfoot, two screens wide, once,
/// twice as fast.
pub(crate) fn journey(ctx: &Ctx, cells: &mut [Cell]) -> Journeyed {
    let sky_limit = layers::paint(ctx, cells, &RANGES[..1]);
    let mid = Strip {
        layer: &RANGES[1],
        cols: ctx.cols,
    };
    let near = Strip {
        layer: &RIDGE,
        cols: 2 * ctx.cols,
    };
    let mut near_cells = near.paint(ctx);
    // Waymarks standing on the ridge line, with wrap-around columns.
    let seed = hash_label(ctx.terrain, "waymarks");
    let spacing = 23u32;
    let mut strip = StripCells {
        cells: &mut near_cells,
        cols: near.cols,
        rows: ctx.rows,
    };
    let mut col = 0;
    while col < near.cols {
        let k = hash3(seed, i64::from(col), 3);
        let c = col + ((k >> 20) % u64::from(spacing)) as u32;
        if unit_from_hash(k) < 0.45 && c < near.cols {
            let row = layers::row_at(ctx, near.top(ctx, f64::from(c) + 0.5)) - 1;
            let shape = WAYMARKS[((k >> 40) % WAYMARKS.len() as u64) as usize];
            strip.stamp(shape, i64::from(c), row, 0.7);
        }
        col += spacing;
    }
    // The ground's height at every pixel column of the ridge's strip.
    let cw = f64::from(ctx.co.g.cell_w);
    let surface = (0..near.cols * ctx.co.g.cell_w)
        .map(|x| near.top(ctx, (f64::from(x) + 0.5) / cw) * ctx.co.h)
        .collect();
    Journeyed {
        sky_limit,
        layers: vec![
            ScrollLayer {
                cols: mid.cols,
                turns: 1,
                cells: mid.paint(ctx),
            },
            ScrollLayer {
                cols: near.cols,
                turns: 1,
                cells: near_cells,
            },
        ],
        surface,
    }
}
