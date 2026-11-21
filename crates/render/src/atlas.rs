//! Glyph atlas: every glyph of `glyphs::GLYPHS` rasterized once into a cell-sized
//! alpha mask.
//!
//! The rasterizer is our own: `skrifa` gives unscaled, unhinted outlines in font
//! units, and coverage is an 8x8 supersampled non-zero winding test. It only uses
//! `+ - * /` and comparisons on f64, which are IEEE-exact, so masks are
//! byte-identical on every OS and CPU. (A SIMD rasterizer with separate x86 and
//! ARM paths would not guarantee that.)

use clairlune_scene::GridSpec;
use clairlune_scene::glyphs::GLYPHS;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

/// The bundled monospace font (JetBrains Mono Regular, SIL OFL 1.1).
pub static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

/// Samples per pixel along each axis.
const SUPERSAMPLE: u32 = 8;
/// Line segments per quadratic / cubic curve.
const QUAD_STEPS: u32 = 8;
const CUBIC_STEPS: u32 = 12;
/// Room below the baseline reserved for descenders, in em.
const DESCENT_ROOM_EM: f64 = 0.22;

/// Alpha masks for every glyph at one cell size.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GlyphAtlas {
    pub cell_w: u32,
    pub cell_h: u32,
    masks: Vec<u8>,
}

#[derive(Debug)]
pub enum AtlasError {
    Font(skrifa::raw::ReadError),
}

impl std::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtlasError::Font(e) => write!(f, "font parse error: {e}"),
        }
    }
}

impl std::error::Error for AtlasError {}

impl GlyphAtlas {
    /// Atlas for a grid, using the bundled font.
    pub fn for_grid(spec: &GridSpec) -> Self {
        Self::with_font(FONT_DATA, spec.cell_w, spec.cell_h, spec.font_px)
            .expect("bundled font parses")
    }

    /// Atlas from arbitrary font bytes.
    pub fn with_font(
        font: &[u8],
        cell_w: u32,
        cell_h: u32,
        font_px: f64,
    ) -> Result<Self, AtlasError> {
        let face = FontRef::new(font).map_err(AtlasError::Font)?;
        // Everything in font units; we scale ourselves with f64.
        let location = LocationRef::default();
        let metrics = face.metrics(Size::unscaled(), location);
        let glyph_metrics = face.glyph_metrics(Size::unscaled(), location);
        let outlines = face.outline_glyphs();
        let charmap = face.charmap();
        let upm = f64::from(metrics.units_per_em);
        let scale = font_px / upm;
        let cap = metrics.cap_height.map_or(0.73 * upm, f64::from);
        let content = (cap + DESCENT_ROOM_EM * upm) * scale;
        let baseline = (f64::from(cell_h) - content) * 0.5 + cap * scale;
        let cell_len = (cell_w * cell_h) as usize;
        let mut masks = vec![0u8; cell_len * GLYPHS.len()];
        for (i, &ch) in GLYPHS.iter().enumerate() {
            if ch == ' ' {
                continue;
            }
            let Some(id) = charmap.map(ch) else {
                continue;
            };
            let Some(glyph) = outlines.get(id) else {
                continue;
            };
            let advance = f64::from(glyph_metrics.advance_width(id).unwrap_or(0.0)) * scale;
            let mut outline = Flattener {
                segs: Vec::new(),
                ox: (f64::from(cell_w) - advance) * 0.5,
                baseline,
                scale,
                cur: (0.0, 0.0),
                start: (0.0, 0.0),
            };
            let settings = DrawSettings::unhinted(Size::unscaled(), location);
            if glyph.draw(settings, &mut outline).is_err() {
                continue;
            }
            rasterize(
                &outline.segs,
                cell_w,
                cell_h,
                &mut masks[i * cell_len..(i + 1) * cell_len],
            );
        }
        Ok(Self {
            cell_w,
            cell_h,
            masks,
        })
    }

    /// Whether it is whole glyphs at this cell size, at least the empty one.
    /// (A saved atlas may have fewer glyphs than today's table: an older one.)
    pub fn fits(&self, cell_w: u32, cell_h: u32) -> bool {
        let len = (cell_w as usize).saturating_mul(cell_h as usize);
        (self.cell_w, self.cell_h) == (cell_w, cell_h)
            && len > 0
            && self.masks.len() >= len
            && self.masks.len().is_multiple_of(len)
    }

    /// Row-major alpha mask (`cell_w * cell_h`) of a glyph id. A glyph an
    /// older saved atlas doesn't have draws as the empty cell.
    #[inline]
    pub fn mask(&self, glyph: u8) -> &[u8] {
        let len = (self.cell_w * self.cell_h) as usize;
        let start = glyph as usize * len;
        self.masks
            .get(start..start + len)
            .unwrap_or(&self.masks[..len])
    }
}

#[derive(Clone, Copy, Debug)]
struct Seg {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// Collects an outline as line segments in cell pixel space (y down).
struct Flattener {
    segs: Vec<Seg>,
    ox: f64,
    baseline: f64,
    scale: f64,
    cur: (f64, f64),
    start: (f64, f64),
}

impl Flattener {
    fn map(&self, x: f32, y: f32) -> (f64, f64) {
        (
            self.ox + f64::from(x) * self.scale,
            self.baseline - f64::from(y) * self.scale,
        )
    }

    fn push(&mut self, p: (f64, f64)) {
        if p != self.cur {
            self.segs.push(Seg {
                x0: self.cur.0,
                y0: self.cur.1,
                x1: p.0,
                y1: p.1,
            });
        }
        self.cur = p;
    }
}

impl OutlinePen for Flattener {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.cur = p;
        self.start = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.push(p);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.cur;
        let c = self.map(x1, y1);
        let p1 = self.map(x, y);
        for i in 1..=QUAD_STEPS {
            let t = f64::from(i) / f64::from(QUAD_STEPS);
            let u = 1.0 - t;
            let px = u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p1.0;
            let py = u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p1.1;
            self.push((px, py));
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.cur;
        let c1 = self.map(x1, y1);
        let c2 = self.map(x2, y2);
        let p1 = self.map(x, y);
        for i in 1..=CUBIC_STEPS {
            let t = f64::from(i) / f64::from(CUBIC_STEPS);
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let px = a * p0.0 + b * c1.0 + c * c2.0 + d * p1.0;
            let py = a * p0.1 + b * c1.1 + c * c2.1 + d * p1.1;
            self.push((px, py));
        }
    }

    fn close(&mut self) {
        let s = self.start;
        self.push(s);
    }
}

/// Non-zero winding coverage, `SUPERSAMPLE`² samples per pixel, into `out` (0..=255).
fn rasterize(segs: &[Seg], w: u32, h: u32, out: &mut [u8]) {
    let s = SUPERSAMPLE;
    let mut counts = vec![0u32; (w * h) as usize];
    let mut xs: Vec<(f64, i32)> = Vec::with_capacity(32);
    for py in 0..h {
        for sy in 0..s {
            let y = f64::from(py) + (f64::from(sy) + 0.5) / f64::from(s);
            xs.clear();
            for g in segs {
                let crosses = (g.y0 <= y && y < g.y1) || (g.y1 <= y && y < g.y0);
                if crosses {
                    let t = (y - g.y0) / (g.y1 - g.y0);
                    let dir = if g.y1 > g.y0 { 1 } else { -1 };
                    xs.push((g.x0 + t * (g.x1 - g.x0), dir));
                }
            }
            if xs.is_empty() {
                continue;
            }
            xs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let mut winding = 0;
            let mut k = 0;
            let row = (py * w) as usize;
            for px in 0..w {
                for sx in 0..s {
                    let x = f64::from(px) + (f64::from(sx) + 0.5) / f64::from(s);
                    while k < xs.len() && xs[k].0 <= x {
                        winding += xs[k].1;
                        k += 1;
                    }
                    if winding != 0 {
                        counts[row + px as usize] += 1;
                    }
                }
            }
        }
    }
    let total = s * s;
    for (o, &c) in out.iter_mut().zip(&counts) {
        *o = ((c * 255 + total / 2) / total) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_scene::glyphs::{EMPTY, id};
    use clairlune_scene::grid_spec;

    fn ink(atlas: &GlyphAtlas, g: u8) -> u32 {
        atlas.mask(g).iter().map(|&a| u32::from(a)).sum()
    }

    #[test]
    fn glyphs_have_ink_and_space_is_empty() {
        let atlas = GlyphAtlas::for_grid(&grid_spec(3840, 2160));
        assert_eq!(ink(&atlas, EMPTY), 0);
        for g in 1..GLYPHS.len() as u8 {
            assert!(
                ink(&atlas, g) > 0,
                "glyph {:?} is blank",
                GLYPHS[g as usize]
            );
        }
        // Heavier glyphs carry more ink than light ones.
        assert!(ink(&atlas, id('@')) > ink(&atlas, id('.')));
        assert!(ink(&atlas, id('#')) > ink(&atlas, id('-')));
    }

    #[test]
    fn atlas_is_deterministic() {
        let spec = grid_spec(640, 360);
        assert_eq!(GlyphAtlas::for_grid(&spec), GlyphAtlas::for_grid(&spec));
    }
}
