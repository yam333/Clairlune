//! Desert: flat-topped mesas on the horizon, then rolling dunes whose sunlit
//! and shaded faces hatch differently; the focal group stands on a dune crest.

use super::layers::{self, Layer, Strip};
use super::{Ctx, Journeyed, details};
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
    let on = details::enabled(ctx);
    let sun = if on {
        details::desert_sky(ctx, cells)
    } else {
        None
    };
    let mut sky = layers::paint(ctx, cells, &LAYERS);
    if on {
        details::desert_plants(ctx, cells, &mut sky, &LAYERS[1], &LAYERS[2], sun);
    }
    sky
}

/// The dunes a journey crosses: the far ones slide by slowly, the trail
/// underfoot (low, long swells) twice as fast.
const DUNES_MID: Layer = Layer {
    base: -0.02,
    amp: 0.06,
    freq: 1.6,
    label: "dunes_mid_strip",
    ..LAYERS[1]
};
const DUNE_TRAIL: Layer = Layer {
    base: 0.08,
    amp: 0.045,
    freq: 1.3,
    label: "dune_trail",
    ..LAYERS[2]
};
const ROCK: [&str; 1] = ["o"];
const ROCKS: [&str; 1] = ["oO"];
const BRUSH: [&str; 2] = ["\\|/", " | "];
const CACTUS: [&str; 3] = ["  |  ", "|_|_|", "  |  "];

/// A journey over the dunes: the sky, a sun or a moon low over the mesas,
/// and the mesas stay put (with a landmark, that takes the sky's place);
/// the far dunes slide by, and the trail with its brush, rocks and the odd
/// cactus twice as fast.
pub(crate) fn journey(ctx: &Ctx, cells: &mut [Cell]) -> Journeyed {
    if ctx.recipe.composition.landmark.is_empty() && details::enabled(ctx) {
        details::desert_sky(ctx, cells);
    }
    let sky_limit = layers::paint(ctx, cells, &LAYERS[..1]);
    let mid = Strip {
        layer: &DUNES_MID,
        cols: ctx.cols,
    };
    let mut mid_cells = mid.paint(ctx);
    layers::stamp_strip(
        ctx,
        &mid,
        &mut mid_cells,
        &[&BRUSH, &ROCK],
        17,
        0.45,
        0.55,
        0,
        "dunes_mid_brush",
    );
    let near = Strip {
        layer: &DUNE_TRAIL,
        cols: 2 * ctx.cols,
    };
    let mut near_cells = near.paint(ctx);
    layers::stamp_strip(
        ctx,
        &near,
        &mut near_cells,
        &[&BRUSH, &ROCK, &ROCKS, &CACTUS],
        23,
        0.5,
        0.8,
        0,
        "dune_trail_plants",
    );
    let (layers, surface) = layers::journey_layers(ctx, (&mid, mid_cells), (&near, near_cells));
    Journeyed {
        sky_limit,
        layers,
        surface,
    }
}
