//! Mountains: three ranges of ridged peaks, snow on the high ones, hatching
//! that follows each face; the focal group stands on a rock ledge in front.

use super::Ctx;
use super::layers::{self, Layer, Snow};
use crate::glyphs::{self, id};
use crate::graph::Cell;

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
