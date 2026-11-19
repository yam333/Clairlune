//! Desert: flat-topped mesas on the horizon, then rolling dunes whose sunlit
//! and shaded faces hatch differently; the focal group stands on a dune crest.

use super::Ctx;
use super::layers::{self, Layer};
use crate::glyphs::{self, id};
use crate::graph::Cell;

const STRATA: [u8; 3] = [id('-'), id('='), id('_')];
const LIT: [u8; 3] = [id('`'), id('.'), id(',')];
const SHADED: [u8; 4] = [id('.'), id(':'), id(';'), id('~')];

const LAYERS: [Layer; 3] = [
    Layer {
        family: &STRATA,
        hatch: None,
        base: -0.10,
        amp: 0.11,
        freq: 1.1,
        ridged: false,
        plateau: Some(0.68),
        fill: (6.0, 40.0),
        threshold: 0.32,
        shade: 0.3,
        outline: Some(0.55),
        snow: None,
        label: "mesas",
    },
    Layer {
        family: &glyphs::SAND,
        hatch: Some((&LIT, &SHADED)),
        base: -0.03,
        amp: 0.08,
        freq: 1.8,
        ridged: false,
        plateau: None,
        fill: (5.0, 48.0),
        threshold: 0.45,
        shade: 0.5,
        outline: Some(0.6),
        snow: None,
        label: "dunes_far",
    },
    Layer {
        family: &glyphs::SAND,
        hatch: Some((&LIT, &SHADED)),
        base: 0.08,
        amp: 0.08,
        freq: 2.6,
        ridged: false,
        plateau: None,
        fill: (4.0, 44.0),
        threshold: 0.42,
        shade: 0.7,
        outline: Some(0.75),
        snow: None,
        label: "dunes_near",
    },
];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    layers::paint(ctx, cells, &LAYERS)
}
