//! A generic layer stack for open landscapes (mountains, snowfields, dunes,
//! hills): each layer is a silhouette (ridged peaks or smooth swells) filled with
//! density-mapped glyphs, with holes, a top-edge fade, an optional outline and
//! snow, and dimming below the platform line. A layer hides what is behind it.
//! Plus stamps: small multi-cell shapes such as pine trees placed along a
//! silhouette.

use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs::{GLYPHS, id};
use crate::graph::{Cell, HOLE};
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, smoothstep};
use crate::noise::{fbm1, fbm1_periodic, fbm2, fbm2_periodic_x, ridged1, ridged1_periodic};

/// Snow on the high parts of a layer: the rows under the top where the
/// silhouette rises above `line` (0 = the layer's mean top, 1 = its highest).
#[derive(Clone, Copy)]
pub(crate) struct Snow {
    pub line: f64,
    pub rows: f64,
    pub glyphs: &'static [u8],
    pub shade: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct Layer {
    pub family: &'static [u8],
    /// Families for slopes rising and falling left to right (hatching that
    /// follows the face), or `None` to use `family` everywhere.
    pub hatch: Option<(&'static [u8], &'static [u8])>,
    /// Mean silhouette top relative to the horizon (normalised y, negative is
    /// higher up) and its swing either way.
    pub base: f64,
    pub amp: f64,
    /// Silhouette features per aspect unit across the screen.
    pub freq: f64,
    /// Ridged noise (sharp peaks) or smooth fBm (rolling swells).
    pub ridged: bool,
    /// Plateau: flattens everything above this fraction of the swing (mesas).
    pub plateau: Option<f64>,
    /// Fill noise frequency across (x aspect) and down.
    pub fill: (f64, f64),
    pub threshold: f64,
    pub shade: f64,
    /// Shade of the outline along the top, or `None` for no outline.
    pub outline: Option<f64>,
    pub snow: Option<Snow>,
    /// Seed label (each layer gets its own stream).
    pub label: &'static str,
}

impl Layer {
    /// Silhouette height in 0..1 (1 = highest) at normalised x.
    fn height(&self, ctx: &Ctx, nx: f64) -> f64 {
        let seed = hash_label(ctx.terrain, self.label);
        let x = nx * ctx.aspect * self.freq;
        let n = if self.ridged {
            ridged1(seed, x, 5)
        } else {
            fbm1(seed, x, 4)
        };
        self.shape(n)
    }

    /// Noise in 0..1 shaped into a silhouette height.
    fn shape(&self, n: f64) -> f64 {
        // fBm clusters around 0.5, so stretch it; ridged noise already spans
        // the range and stretching would flatten its peaks.
        let n = if self.ridged {
            n
        } else {
            clamp01((n - 0.5) * 1.6 + 0.5)
        };
        match self.plateau {
            // Mesas: a steep step up to a flat top, low rolling ground between.
            Some(p) => smoothstep(p - 0.04, p + 0.01, n) * 0.85 + n.min(p) / p * 0.15,
            None => n,
        }
    }

    /// Silhouette top (normalised y) at normalised x.
    pub fn top(&self, ctx: &Ctx, nx: f64) -> f64 {
        let n = self.height(ctx, nx);
        (ctx.horizon + self.base - self.amp * (n - 0.5) * 2.0).max(0.04)
    }

    /// The silhouette top in each column, in fractional rows.
    fn top_rows(&self, ctx: &Ctx) -> Vec<f64> {
        (0..ctx.cols)
            .map(|col| row_f(ctx, self.top(ctx, ctx.co.nx(col))))
            .collect()
    }
}

/// A layer painted as a strip `cols` cells wide whose right edge continues
/// into its left edge (docs/dev/scrolling-scenes.md): every noise lookup wraps
/// with the strip, so the silhouette and the fill tile with no seam.
pub(crate) struct Strip<'a> {
    pub layer: &'a Layer,
    pub cols: u32,
}

impl Strip<'_> {
    /// Lattice period for `features` per aspect unit across the screen, so
    /// the strip looks like the still layer at the same frequency.
    fn period(&self, ctx: &Ctx, features: f64) -> i64 {
        (features * ctx.aspect * f64::from(self.cols) / f64::from(ctx.cols))
            .round()
            .max(1.0) as i64
    }

    /// Silhouette height in 0..1 at `x` cells into the strip (any x: wraps).
    fn height(&self, ctx: &Ctx, x: f64) -> f64 {
        let l = self.layer;
        let seed = hash_label(ctx.terrain, l.label);
        let period = self.period(ctx, l.freq);
        let x = x.rem_euclid(f64::from(self.cols)) * period as f64 / f64::from(self.cols);
        let n = if l.ridged {
            ridged1_periodic(seed, x, period, 5)
        } else {
            fbm1_periodic(seed, x, period, 4)
        };
        l.shape(n)
    }

    /// Silhouette top (normalised y) at `x` cells into the strip; a cell's
    /// centre is at `col + 0.5`.
    pub fn top(&self, ctx: &Ctx, x: f64) -> f64 {
        let l = self.layer;
        let n = self.height(ctx, x);
        (ctx.horizon + l.base - l.amp * (n - 0.5) * 2.0).max(0.04)
    }

    /// Paints the strip: `cols * ctx.rows` cells, see-through above the
    /// silhouette, [`HOLE`] where the fill leaves a gap.
    pub fn paint(&self, ctx: &Ctx) -> Vec<Cell> {
        let l = self.layer;
        let (cols, rows) = (self.cols, ctx.rows);
        let mut cells = vec![Cell::default(); (cols * rows) as usize];
        let jitter_seed = hash_label(ctx.terrain, "jitter");
        let fill_seed = hash_label(ctx.terrain, l.label) ^ 0x5_DEEC_E66D;
        let fill_period = self.period(ctx, l.fill.0);
        let rows_f = f64::from(rows);
        let tops: Vec<f64> = (0..cols)
            .map(|c| self.top(ctx, f64::from(c) + 0.5))
            .collect();
        for col in 0..cols {
            let top = tops[col as usize];
            let family = match l.hatch {
                Some((rising, falling)) => {
                    let left = tops[((col + cols - 1) % cols) as usize];
                    let right = tops[((col + 1) % cols) as usize];
                    if right < left { rising } else { falling }
                }
                None => l.family,
            };
            let snowy = l
                .snow
                .filter(|s| self.height(ctx, f64::from(col) + 0.5) >= 0.5 + 0.5 * s.line);
            let fx = (f64::from(col) + 0.5) * fill_period as f64 / f64::from(cols);
            for row in 0..rows {
                let ny = ctx.co.ny(row);
                if ny < top {
                    continue;
                }
                let depth = ny - top;
                let rows_below_top = depth * rows_f;
                let fade = 0.55 + 0.45 * (rows_below_top / 6.0).min(1.0);
                let idx = (row * cols + col) as usize;
                if let Some(s) = snowy
                    && rows_below_top < s.rows
                {
                    let k = hash3(jitter_seed ^ fill_seed, i64::from(col), i64::from(row));
                    cells[idx] = Cell {
                        glyph: s.glyphs[(k % s.glyphs.len() as u64) as usize],
                        shade: shade_u8(s.shade),
                    };
                    continue;
                }
                let n = fbm2_periodic_x(fill_seed, fx, ny * l.fill.1, fill_period, 4);
                let mut d = clamp01((n - 0.5) * 1.9 + 0.5);
                d *= 0.80 + 0.20 * smoothstep(0.0, 0.10, depth);
                cells[idx] = if d < l.threshold {
                    HOLE
                } else {
                    Cell {
                        glyph: family_glyph(
                            family,
                            d,
                            l.threshold,
                            jitter_seed ^ fill_seed,
                            col,
                            row,
                        ),
                        shade: shade_u8(l.shade * fade),
                    }
                };
            }
        }
        if let Some(shade) = l.outline {
            let tops_rows: Vec<f64> = tops.iter().map(|&t| row_f(ctx, t)).collect();
            let mut grid = StripCells {
                cells: &mut cells,
                cols,
                rows,
            };
            outline_with(&tops_rows, true, shade, &mut |c, r, g, s| {
                grid.put(c, r, g, s)
            });
        }
        cells
    }
}

/// A strip's cells, written with wrap-around columns.
pub(crate) struct StripCells<'a> {
    pub cells: &'a mut [Cell],
    pub cols: u32,
    pub rows: u32,
}

impl StripCells<'_> {
    pub fn put(&mut self, col: i64, row: i64, glyph: u8, shade: f64) {
        if row < 0 || row >= i64::from(self.rows) {
            return;
        }
        let col = col.rem_euclid(i64::from(self.cols));
        self.cells[(row * i64::from(self.cols) + col) as usize] = Cell {
            glyph,
            shade: shade_u8(shade),
        };
    }

    /// A text stamp with its bottom-centre on `(col, row)`; spaces skipped.
    pub fn stamp(&mut self, shape: &[&str], col: i64, row: i64, shade: f64) {
        let h = shape.len() as i64;
        for (dy, line) in shape.iter().enumerate() {
            let w = line.chars().count() as i64;
            for (dx, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    self.put(
                        col - w / 2 + dx as i64,
                        row - h + 1 + dy as i64,
                        glyph_of(ch),
                        shade,
                    );
                }
            }
        }
    }
}

/// Normalised y in fractional rows (row `r` spans `r..r + 1`).
pub(crate) fn row_f(ctx: &Ctx, y: f64) -> f64 {
    let g = &ctx.co.g;
    (y * ctx.co.h - f64::from(g.origin_y)) / f64::from(g.cell_h)
}

/// The first row whose centre is at or below normalised y.
pub(crate) fn row_at(ctx: &Ctx, y: f64) -> i64 {
    (row_f(ctx, y) - 0.5).ceil() as i64
}

/// Paints layers back to front; returns the highest silhouette per column.
pub(crate) fn paint(ctx: &Ctx, cells: &mut [Cell], layers: &[Layer]) -> Vec<f64> {
    let co = &ctx.co;
    let mut sky_limit = vec![f64::INFINITY; ctx.cols as usize];
    let jitter_seed = hash_label(ctx.terrain, "jitter");
    let rows_f = f64::from(ctx.rows);
    for layer in layers {
        let fill_seed = hash_label(ctx.terrain, layer.label) ^ 0x5_DEEC_E66D;
        let tops = layer.top_rows(ctx);
        for col in 0..ctx.cols {
            let nx = co.nx(col);
            let top = layer.top(ctx, nx);
            sky_limit[col as usize] = sky_limit[col as usize].min(top);
            // The local slope picks the hatch family.
            let family = match layer.hatch {
                Some((rising, falling)) => {
                    let d = 2.0 / f64::from(ctx.cols);
                    let slope = layer.top(ctx, nx + d) - layer.top(ctx, nx - d);
                    if slope < 0.0 { rising } else { falling }
                }
                None => layer.family,
            };
            let snowy = layer
                .snow
                .filter(|s| layer.height(ctx, nx) >= 0.5 + 0.5 * s.line);
            for row in 0..ctx.rows {
                let ny = co.ny(row);
                if ny < top {
                    continue;
                }
                let depth = ny - top;
                let rows_below_top = depth * rows_f;
                let fade = 0.55 + 0.45 * (rows_below_top / 6.0).min(1.0);
                let mut shade = layer.shade * fade;
                if row > ctx.platform_row {
                    shade *= 0.5;
                }
                let idx = ctx.idx(col, row);
                if let Some(s) = snowy
                    && rows_below_top < s.rows
                {
                    let k = hash3(jitter_seed ^ fill_seed, i64::from(col), i64::from(row));
                    cells[idx] = Cell {
                        glyph: s.glyphs[(k % s.glyphs.len() as u64) as usize],
                        shade: shade_u8(s.shade),
                    };
                    continue;
                }
                let n = fbm2(
                    fill_seed,
                    nx * ctx.aspect * layer.fill.0,
                    ny * layer.fill.1,
                    4,
                );
                let mut d = clamp01((n - 0.5) * 1.9 + 0.5);
                d *= 0.80 + 0.20 * smoothstep(0.0, 0.10, depth);
                cells[idx] = if d < layer.threshold {
                    Cell::default() // a hole: dark, and hides the layer behind
                } else {
                    Cell {
                        glyph: family_glyph(
                            family,
                            d,
                            layer.threshold,
                            jitter_seed ^ fill_seed,
                            col,
                            row,
                        ),
                        shade: shade_u8(shade),
                    }
                };
            }
        }
        if let Some(shade) = layer.outline {
            outline(ctx, cells, &tops, shade);
        }
    }
    sky_limit
}

/// Writes one cell if it is on screen, dimmed below the platform line.
pub(crate) fn put(ctx: &Ctx, cells: &mut [Cell], col: i64, row: i64, glyph: u8, shade: f64) {
    if col < 0 || row < 0 || col >= i64::from(ctx.cols) || row >= i64::from(ctx.rows) {
        return;
    }
    let s = if row > i64::from(ctx.platform_row) {
        shade * 0.5
    } else {
        shade
    };
    cells[ctx.idx(col as u32, row as u32)] = Cell {
        glyph,
        shade: shade_u8(s),
    };
}

/// An outline along a silhouette given in fractional rows per column. Gentle
/// stretches pick `_`, `-` or `.` by where the line falls inside the cell (a
/// line near a cell's top edge is drawn as `_` in the row above), so shallow
/// curves stay smooth; steep ones use `/` and `\`, local tops `^`. Where the
/// top jumps several rows between neighbours, the lower column carries a run
/// of slope glyphs up to the higher one, `|` when it is a wall.
fn outline(ctx: &Ctx, cells: &mut [Cell], tops: &[f64], shade: f64) {
    outline_with(tops, false, shade, &mut |c, r, g, s| {
        put(ctx, cells, c, r, g, s)
    });
}

/// [`outline`] through a writer; with `wrap` the first and last columns are
/// neighbours (a strip that tiles).
fn outline_with(tops: &[f64], wrap: bool, shade: f64, put: &mut dyn FnMut(i64, i64, u8, f64)) {
    const WALL: i64 = 3;
    let n = tops.len();
    let at = |c: usize| tops[c];
    let prev = |c: usize| {
        if wrap {
            (c + n - 1) % n
        } else {
            c.saturating_sub(1)
        }
    };
    let next = |c: usize| {
        if wrap {
            (c + 1) % n
        } else {
            (c + 1).min(n - 1)
        }
    };
    let mut placed = vec![0i64; n];
    let mut glyphs = vec![0u8; n];
    for c in 0..n {
        let y = at(c);
        let left = at(prev(c));
        let right = at(next(c));
        let slope = right - left; // rows over two columns; negative is rising
        let row = y.floor();
        let f = y - row;
        let (glyph, r) = if y < left - 0.5 && y < right - 0.5 {
            (id('^'), row)
        } else if slope < -1.2 {
            (id('/'), row)
        } else if slope > 1.2 {
            (id('\\'), row)
        } else if f < 0.27 {
            (id('_'), row - 1.0)
        } else if f < 0.67 {
            (id('-'), row)
        } else if f < 0.87 {
            (id('.'), row)
        } else {
            (id('_'), row)
        };
        placed[c] = r as i64;
        glyphs[c] = glyph;
    }
    for c in 0..n {
        let r = placed[c];
        put(c as i64, r, glyphs[c], shade);
        let (right, left) = (next(c), prev(c));
        // A higher right neighbour: climb toward it in this column.
        if (wrap || c + 1 < n) && placed[right] < r - 1 {
            let g = if r - placed[right] > WALL {
                id('|')
            } else {
                id('/')
            };
            for row in placed[right] + 1..r {
                put(c as i64, row, g, shade);
            }
        }
        // A higher left neighbour: come down from it in this column.
        if (wrap || c > 0) && placed[left] < r - 1 {
            let g = if r - placed[left] > WALL {
                id('|')
            } else {
                id('\\')
            };
            for row in placed[left] + 1..r {
                put(c as i64, row, g, shade);
            }
        }
    }
}

/// Glyph id of a character (stamps are written as text).
fn glyph_of(c: char) -> u8 {
    GLYPHS
        .iter()
        .position(|&g| g == c)
        .expect("stamp glyph in GLYPHS") as u8
}

/// Draws a text stamp with its bottom-centre on `(col, row)`. Spaces are skipped.
pub(crate) fn stamp(ctx: &Ctx, cells: &mut [Cell], shape: &[&str], col: i64, row: i64, shade: f64) {
    let h = shape.len() as i64;
    for (dy, line) in shape.iter().enumerate() {
        let w = line.chars().count() as i64;
        for (dx, ch) in line.chars().enumerate() {
            if ch != ' ' {
                let c = col - w / 2 + dx as i64;
                let r = row - h + 1 + dy as i64;
                put(ctx, cells, c, r, glyph_of(ch), shade);
            }
        }
    }
}

/// Scatters stamps along a layer's silhouette: roughly one per `spacing` columns,
/// skipping some at random, with the shape picked per stamp. A stamp's last
/// line sits on the top row.
pub(crate) fn stamp_along(
    ctx: &Ctx,
    cells: &mut [Cell],
    layer: &Layer,
    shapes: &[&[&str]],
    spacing: u32,
    shade: f64,
    label: &str,
) {
    let seed = hash_label(ctx.terrain, label);
    let spacing = spacing.max(1);
    let mut col = 0u32;
    while col < ctx.cols {
        let k = hash3(seed, i64::from(col), 7);
        let skip = unit_from_hash(k) < 0.35;
        let c = col + ((k >> 20) % u64::from(spacing)) as u32;
        if !skip && c < ctx.cols {
            let row = row_at(ctx, layer.top(ctx, ctx.co.nx(c)));
            let shape = shapes[((k >> 40) % shapes.len() as u64) as usize];
            stamp(ctx, cells, shape, i64::from(c), row, shade);
        }
        col += spacing;
    }
}

/// Text stamps along a strip's silhouette, about one per `spacing` columns
/// (a share `keep` of them), wrapping with the strip. `lift` rows up from
/// the silhouette's row: 0 stands the stamp's last line on it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stamp_strip(
    ctx: &Ctx,
    strip: &Strip,
    cells: &mut [Cell],
    shapes: &[&[&str]],
    spacing: u32,
    keep: f64,
    shade: f64,
    lift: i64,
    label: &str,
) {
    let seed = hash_label(ctx.terrain, label);
    let spacing = spacing.max(1);
    let mut out = StripCells {
        cells,
        cols: strip.cols,
        rows: ctx.rows,
    };
    let mut col = 0;
    while col < strip.cols {
        let k = hash3(seed, i64::from(col), 3);
        let c = col + ((k >> 20) % u64::from(spacing)) as u32;
        if unit_from_hash(k) < keep && c < strip.cols {
            let row = row_at(ctx, strip.top(ctx, f64::from(c) + 0.5)) - lift;
            let shape = shapes[((k >> 40) % shapes.len() as u64) as usize];
            out.stamp(shape, i64::from(c), row, shade);
        }
        col += spacing;
    }
}

/// A journey's two moving layers, the middle one (a screen wide, once per
/// loop) and the one walked on (two screens wide, also once: twice as fast),
/// with the walking layer's ground at every pixel column of its strip.
pub(crate) fn journey_layers(
    ctx: &Ctx,
    mid: (&Strip, Vec<Cell>),
    near: (&Strip, Vec<Cell>),
) -> (Vec<crate::graph::ScrollLayer>, Vec<f64>) {
    let cw = f64::from(ctx.co.g.cell_w);
    let surface = (0..near.0.cols * ctx.co.g.cell_w)
        .map(|x| near.0.top(ctx, (f64::from(x) + 0.5) / cw) * ctx.co.h)
        .collect();
    let layers = vec![
        crate::graph::ScrollLayer {
            cols: mid.0.cols,
            turns: 1,
            cells: mid.1,
        },
        crate::graph::ScrollLayer {
            cols: near.0.cols,
            turns: 1,
            cells: near.1,
        },
    ];
    (layers, surface)
}
