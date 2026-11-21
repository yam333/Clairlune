//! Snowfield: a distant snowy range, sparse rolling drifts and scattered pines;
//! the focal group stands on an ice shelf.

use super::layers::{self, Layer, Snow, Strip};
use super::{Ctx, Journeyed};
use crate::glyphs::{self, id};
use crate::graph::Cell;

const SNOWCAP: [u8; 3] = [id('.'), id(':'), id('\'')];

const PINE_FAR: &[&str] = &["^", "|"];
const PINE_SMALL: &[&str] = &["^", "/^\\", "|"];
const PINE_TALL: &[&str] = &["^", "/^\\", "/^^^\\", "|"];
const PINE_BIG: &[&str] = &["^", "/^\\", "/^^^\\", "/^^^^^\\", "|"];

const FAR: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.22,
    amp: 0.11,
    freq: 1.0,
    ridged: true,
    plateau: None,
    fill: (30.0, 14.0),
    threshold: 0.45,
    shade: 0.3,
    outline: Some(0.5),
    snow: Some(Snow {
        line: 0.0,
        rows: 2.0,
        glyphs: &SNOWCAP,
        shade: 0.55,
    }),
    label: "snow_far",
};
const MID: Layer = Layer {
    family: &glyphs::SNOW,
    hatch: None,
    base: -0.06,
    amp: 0.04,
    freq: 1.5,
    ridged: false,
    plateau: None,
    fill: (22.0, 26.0),
    threshold: 0.58,
    shade: 0.45,
    outline: Some(0.55),
    snow: None,
    label: "snow_mid",
};
const NEAR: Layer = Layer {
    family: &glyphs::SNOW,
    hatch: None,
    base: 0.07,
    amp: 0.03,
    freq: 2.2,
    ridged: false,
    plateau: None,
    fill: (18.0, 22.0),
    threshold: 0.6,
    shade: 0.6,
    outline: Some(0.65),
    snow: None,
    label: "snow_near",
};

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let sky = layers::paint(ctx, cells, &[FAR, MID, NEAR]);
    layers::stamp_along(ctx, cells, &FAR, &[PINE_FAR], 13, 0.4, "pines_far");
    layers::stamp_along(
        ctx,
        cells,
        &MID,
        &[PINE_SMALL, PINE_TALL, PINE_BIG],
        7,
        0.7,
        "pines_mid",
    );
    sky
}

/// The trail a journey glides along: low rolling snow, a little livelier
/// than the still scene's drifts.
const TRAIL: Layer = Layer {
    family: &glyphs::SNOW,
    hatch: None,
    base: 0.08,
    amp: 0.05,
    freq: 1.4,
    ridged: false,
    plateau: None,
    fill: (18.0, 22.0),
    threshold: 0.6,
    shade: 0.6,
    outline: Some(0.7),
    snow: None,
    label: "snow_trail",
};

/// A journey across the snowfield: the far range and its pines stay put,
/// the drifts with their pines slide by, and the trail underfoot (with the
/// odd young pine beside it) twice as fast.
pub(crate) fn journey(ctx: &Ctx, cells: &mut [Cell]) -> Journeyed {
    let sky_limit = layers::paint(ctx, cells, &[FAR]);
    layers::stamp_along(ctx, cells, &FAR, &[PINE_FAR], 13, 0.4, "pines_far");
    let mid = Strip {
        layer: &MID,
        cols: ctx.cols,
    };
    let mut mid_cells = mid.paint(ctx);
    layers::stamp_strip(
        ctx,
        &mid,
        &mut mid_cells,
        &[PINE_SMALL, PINE_TALL, PINE_BIG],
        7,
        0.65,
        0.7,
        0,
        "pines_mid",
    );
    let near = Strip {
        layer: &TRAIL,
        cols: 2 * ctx.cols,
    };
    let mut near_cells = near.paint(ctx);
    layers::stamp_strip(
        ctx,
        &near,
        &mut near_cells,
        &[PINE_SMALL, PINE_TALL],
        29,
        0.35,
        0.8,
        0,
        "pines_trail",
    );
    let (layers, surface) = layers::journey_layers(ctx, (&mid, mid_cells), (&near, near_cells));
    Journeyed {
        sky_limit,
        layers,
        surface,
    }
}
