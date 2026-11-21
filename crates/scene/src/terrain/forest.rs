//! Forest: rolling hills far off, a scalloped treeline of small crowns, two
//! rows of round-crowned trees in the middle distance (kept clear of the focal
//! group) and big trees framing the edges; the focal group stands on a grassy
//! bank.

use super::layers::{self, Layer, Strip, StripCells};
use super::{Ctx, Journeyed, family_glyph};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::fbm2;

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.17,
    amp: 0.06,
    freq: 1.0,
    ridged: false,
    plateau: None,
    fill: (24.0, 20.0),
    threshold: 0.5,
    shade: 0.28,
    outline: Some(0.42),
    snow: None,
    label: "forest_hills",
};
const FLOOR: Layer = Layer {
    family: &glyphs::EARTH,
    hatch: None,
    base: 0.03,
    amp: 0.01,
    freq: 1.5,
    ridged: false,
    plateau: None,
    fill: (30.0, 24.0),
    threshold: 0.62,
    shade: 0.42,
    outline: None,
    snow: None,
    label: "forest_floor",
};

/// Bark: light to heavy, for the inside of wide trunks.
const BARK: [u8; 4] = [id('.'), id(':'), id('|'), id('!')];

/// One tree, in grid units: crown centre column, ground row under the trunk,
/// crown radii (columns, rows) and visible trunk length (rows).
struct Tree {
    cx: f64,
    ground: f64,
    rx: f64,
    ry: f64,
    trunk: f64,
    shade: f64,
    seed: u64,
}

/// A row of trees across the screen.
#[derive(Clone, Copy)]
struct TreeRow {
    label: &'static str,
    /// Crown half-height as a fraction of the screen's short side, before the
    /// per-tree variation of 0.7x to 1.3x.
    size: f64,
    shade: f64,
    /// Ground row relative to the platform row.
    ground: f64,
    /// Trunk length in crown half-heights: minimum and random extra.
    trunk: (f64, f64),
    /// Distance to the next tree in crown half-widths: minimum and random extra.
    spacing: (f64, f64),
    /// Share of trees left out.
    gaps: f64,
    /// Leave the focal group's columns free.
    avoid_group: bool,
}

const ROWS: [TreeRow; 3] = [
    TreeRow {
        label: "forest_treeline",
        size: 0.03,
        shade: 0.3,
        ground: -0.5,
        trunk: (0.0, 0.3),
        spacing: (0.9, 0.7),
        gaps: 0.0,
        avoid_group: false,
    },
    TreeRow {
        label: "forest_mid_far",
        size: 0.06,
        shade: 0.42,
        ground: 0.0,
        trunk: (0.6, 0.6),
        spacing: (1.3, 1.6),
        gaps: 0.2,
        avoid_group: true,
    },
    TreeRow {
        label: "forest_mid_near",
        size: 0.09,
        shade: 0.55,
        ground: 1.5,
        trunk: (0.7, 0.6),
        spacing: (1.6, 2.2),
        gaps: 0.3,
        avoid_group: true,
    },
];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = layers::paint(ctx, cells, &[HILLS, FLOOR]);
    for row in &ROWS {
        for t in tree_row(ctx, row) {
            draw_tree(ctx, cells, &mut sky, &t);
        }
    }
    // Big trees framing the edges, crowns reaching in from above.
    let seed = hash_label(ctx.terrain, "forest_frame");
    for (i, side) in [0.03, 0.97].into_iter().enumerate() {
        let k = hash3(seed, i as i64, 0);
        if pick(k, 0) < 0.25 {
            continue;
        }
        let ry = (0.13 + 0.05 * pick(k, 11)) * rows_per_unit(ctx);
        let rx = ry * aspect_of_cells(ctx) * 1.15;
        let cx = (side + (pick(k, 22) - 0.5) * 0.04) * f64::from(ctx.cols);
        let top = (0.04 + 0.06 * pick(k, 33)) * f64::from(ctx.rows);
        let t = Tree {
            cx,
            ground: f64::from(ctx.rows),
            rx,
            ry,
            trunk: f64::from(ctx.rows) - (top + 2.0 * ry),
            shade: 0.62,
            seed: k,
        };
        if clear_of_group(ctx, &t) {
            draw_tree(ctx, cells, &mut sky, &t);
        }
    }
    sky
}

/// An independent value in `[0, 1)` per `salt` from one tree's hash.
fn pick(k: u64, salt: i64) -> f64 {
    unit_from_hash(hash3(k, salt, 0x7EE))
}

/// Rows of the grid per `unit` (the screen's short side).
fn rows_per_unit(ctx: &Ctx) -> f64 {
    ctx.unit / f64::from(ctx.co.g.cell_h)
}

/// Columns per row of equal pixel length (cells are taller than wide).
fn aspect_of_cells(ctx: &Ctx) -> f64 {
    f64::from(ctx.co.g.cell_h) / f64::from(ctx.co.g.cell_w)
}

/// Whether a tree's crown keeps clear of the focal group's columns.
fn clear_of_group(ctx: &Ctx, t: &Tree) -> bool {
    let g = &ctx.co.g;
    let px = |col: f64| f64::from(g.origin_x) + col * f64::from(g.cell_w);
    let margin = 4.0 * f64::from(g.cell_w);
    px(t.cx + t.rx) < f64::from(ctx.gx0) - margin || px(t.cx - t.rx) > f64::from(ctx.gx1) + margin
}

/// The trees of one row, left to right with irregular spacing.
fn tree_row(ctx: &Ctx, row: &TreeRow) -> Vec<Tree> {
    let seed = hash_label(ctx.terrain, row.label);
    let ground = f64::from(ctx.platform_row) + row.ground;
    let mut trees = Vec::new();
    let mut cx = -2.0;
    let mut i = 0i64;
    while cx < f64::from(ctx.cols) + 2.0 {
        let k = hash3(seed, i, 1);
        let ry = row.size * (0.7 + 0.6 * pick(k, 0)) * rows_per_unit(ctx);
        let rx = ry * aspect_of_cells(ctx) * (0.95 + 0.3 * pick(k, 9));
        let t = Tree {
            cx,
            ground: ground + (pick(k, 18) * 1.5).floor(),
            rx,
            ry,
            trunk: ry * (row.trunk.0 + row.trunk.1 * pick(k, 27)),
            shade: row.shade,
            seed: k,
        };
        if pick(k, 36) >= row.gaps && (!row.avoid_group || clear_of_group(ctx, &t)) {
            trees.push(t);
        }
        cx += rx * (row.spacing.0 + row.spacing.1 * pick(k, 45));
        i += 1;
    }
    trees
}

/// Trunk, then a lumpy round crown filled with foliage glyphs, lit from above.
/// The crown hides what is behind it and lowers the sky limit.
fn draw_tree(ctx: &Ctx, cells: &mut [Cell], sky: &mut [f64], t: &Tree) {
    let cols = ctx.cols;
    draw_tree_with(t, &mut |c, r, glyph, shade, crown| {
        if crown && r >= 0 && c >= 0 && (c as u32) < cols {
            let ny = ctx.co.ny(r as u32) - 0.5 / f64::from(ctx.rows);
            sky[c as usize] = sky[c as usize].min(ny);
        }
        layers::put(ctx, cells, c, r, glyph, shade);
    });
}

/// [`draw_tree`] through a writer (a screen, or a journey's strip), told
/// whether each cell is in the crown (gaps included).
fn draw_tree_with(t: &Tree, put: &mut dyn FnMut(i64, i64, u8, f64, bool)) {
    let cy = t.ground - t.trunk - t.ry;
    // Trunk: one column of `|` for small trees; wider trunks get `|` edges and
    // bark between, flaring by a column at the foot.
    let width = (t.rx * 0.16).round().max(1.0) as i64;
    let top = (cy + t.ry * 0.5).round() as i64;
    let foot = t.ground.round() as i64;
    for r in top..foot {
        let flare = i64::from(width > 1 && r >= foot - 2);
        let c0 = t.cx.round() as i64 - width / 2 - flare;
        let c1 = c0 + width - 1 + 2 * flare;
        for c in c0..=c1 {
            let glyph = if c == c0 || c == c1 {
                id('|')
            } else {
                let k = hash3(t.seed, c, r);
                BARK[(k % BARK.len() as u64) as usize]
            };
            put(c, r, glyph, t.shade * 0.75, false);
        }
    }
    let rows = (cy - t.ry * 1.2).floor() as i64..=(cy + t.ry * 1.2).ceil() as i64;
    for r in rows {
        let cols = (t.cx - t.rx * 1.2).floor() as i64..=(t.cx + t.rx * 1.2).ceil() as i64;
        for c in cols {
            let u = (c as f64 + 0.5 - t.cx) / t.rx;
            let v = (r as f64 + 0.5 - cy) / t.ry;
            let lump = 0.82 + 0.36 * fbm2(t.seed, u * 1.7 + 9.0, v * 1.7, 2);
            if u * u + v * v > lump * lump {
                continue;
            }
            let n = fbm2(t.seed ^ 0x5EED, c as f64 * 0.45, r as f64 * 0.8, 3);
            let d = clamp01((n - 0.5) * 1.8 + 0.6 - 0.2 * v);
            if d < 0.32 {
                put(c, r, EMPTY, 0.0, true);
                continue;
            }
            let glyph = family_glyph(
                &glyphs::FOLIAGE,
                d,
                0.32,
                t.seed,
                c.max(0) as u32,
                r.max(0) as u32,
            );
            let shade = t.shade * (1.0 - 0.3 * v.max(0.0));
            put(c, r, glyph, shade, true);
        }
    }
}

/// The forest floor a journey walks: a gently rolling path.
const PATH: Layer = Layer {
    family: &glyphs::EARTH,
    hatch: None,
    base: 0.08,
    amp: 0.03,
    freq: 1.2,
    ridged: false,
    plateau: None,
    fill: (30.0, 24.0),
    threshold: 0.6,
    shade: 0.45,
    outline: Some(0.6),
    snow: None,
    label: "forest_path",
};
/// The wood beyond the path: a low floor the middle trees stand on.
const WOOD: Layer = Layer {
    family: &glyphs::EARTH,
    hatch: None,
    base: 0.0,
    amp: 0.02,
    freq: 1.0,
    ridged: false,
    plateau: None,
    fill: (30.0, 24.0),
    threshold: 0.66,
    shade: 0.35,
    outline: None,
    snow: None,
    label: "forest_wood",
};

/// Trees along a strip, standing on its silhouette, wrapping with it.
fn strip_trees(ctx: &Ctx, strip: &Strip, row: &TreeRow) -> Vec<Tree> {
    let seed = hash_label(ctx.terrain, row.label);
    let mut trees = Vec::new();
    let mut cx = 0.0;
    let mut i = 0i64;
    while cx < f64::from(strip.cols) {
        let k = hash3(seed, i, 1);
        let ry = row.size * (0.7 + 0.6 * pick(k, 0)) * rows_per_unit(ctx);
        let rx = ry * aspect_of_cells(ctx) * (0.95 + 0.3 * pick(k, 9));
        let ground = layers::row_f(ctx, strip.top(ctx, cx + 0.5)).ceil() + row.ground;
        let t = Tree {
            cx,
            ground,
            rx,
            ry,
            trunk: ry * (row.trunk.0 + row.trunk.1 * pick(k, 27)),
            shade: row.shade,
            seed: k,
        };
        if pick(k, 36) >= row.gaps {
            trees.push(t);
        }
        cx += rx * (row.spacing.0 + row.spacing.1 * pick(k, 45));
        i += 1;
    }
    trees
}

/// A walk through the forest: the hills and the far treeline stay put, the
/// wood's trees slide by, and the path underfoot (big trees beside it now
/// and then) twice as fast.
pub(crate) fn journey(ctx: &Ctx, cells: &mut [Cell]) -> Journeyed {
    let mut sky_limit = layers::paint(ctx, cells, &[HILLS]);
    for t in tree_row(ctx, &ROWS[0]) {
        draw_tree(ctx, cells, &mut sky_limit, &t);
    }
    let paint = |strip: &Strip, row: &TreeRow| {
        let mut cells = strip.paint(ctx);
        let mut out = StripCells {
            cells: &mut cells,
            cols: strip.cols,
            rows: ctx.rows,
        };
        for t in strip_trees(ctx, strip, row) {
            draw_tree_with(&t, &mut |c, r, g, s, _| out.put(c, r, g, s));
        }
        cells
    };
    let mid = Strip {
        layer: &WOOD,
        cols: ctx.cols,
    };
    let near = Strip {
        layer: &PATH,
        cols: 2 * ctx.cols,
    };
    let mid_cells = paint(&mid, &ROWS[1]);
    let near_trees = TreeRow {
        label: "forest_path_trees",
        gaps: 0.55,
        ground: 0.0,
        ..ROWS[2]
    };
    let near_cells = paint(&near, &near_trees);
    let (layers, surface) = layers::journey_layers(ctx, (&mid, mid_cells), (&near, near_cells));
    Journeyed {
        sky_limit,
        layers,
        surface,
    }
}
