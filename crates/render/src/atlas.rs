//! Glyph atlas: every glyph of `glyphs::GLYPHS` rasterized once into a cell-sized
//! alpha mask.
//!
//! The rasterizer is our own: `skrifa` gives unscaled, unhinted outlines in font
//! units, and coverage is an 8x8 supersampled non-zero winding test. It only uses
//! `+ - * /` and comparisons on f64, which are IEEE-exact, so masks are
//! byte-identical on every OS and CPU. (A SIMD rasterizer with separate x86 and
//! ARM paths would not guarantee that.)

use clairlune_scene::glyphs::GLYPHS;
use clairlune_scene::{GridSpec, SceneGraph};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

/// The bundled monospace font (JetBrains Mono Regular, SIL OFL 1.1).
pub static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

/// The katakana the `kana` letter set is drawn in: M PLUS 1 Code's, cut
/// down to them (SIL OFL 1.1, assets/fonts/ClairluneKana-OFL.txt).
pub static KANA_FONT: &[u8] = include_bytes!("../assets/fonts/ClairluneKana.ttf");

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

    /// The atlas a scene's glyphs are drawn with: in its letter set.
    pub fn for_graph(graph: &SceneGraph) -> Self {
        Self::for_letters(&graph.grid, &graph.letters)
    }

    /// Atlas for a grid in a letter set (`clairlune_scene::glyphs::LETTERS`):
    /// the bundled font's characters, katakana from the kana font, narrowed
    /// to the cell, and braille drawn as dots.
    pub fn for_letters(spec: &GridSpec, letters: &str) -> Self {
        let chars = clairlune_scene::glyphs::letters(letters);
        let (w, h) = (spec.cell_w, spec.cell_h);
        let mut atlas = Self::rasterize_chars(FONT_DATA, &chars, w, h, spec.font_px, false)
            .expect("bundled font parses");
        let len = (w * h) as usize;
        let kana: Vec<char> = chars
            .iter()
            .map(|&c| if is_kana(c) { c } else { ' ' })
            .collect();
        if kana.iter().any(|&c| c != ' ') {
            let k = Self::rasterize_chars(KANA_FONT, &kana, w, h, spec.font_px, true)
                .expect("bundled kana font parses");
            for (i, _) in kana.iter().enumerate().filter(|(_, c)| **c != ' ') {
                atlas.masks[i * len..(i + 1) * len]
                    .copy_from_slice(&k.masks[i * len..(i + 1) * len]);
            }
        }
        for (i, &c) in chars.iter().enumerate() {
            if let Some(dots) = braille_dots(c) {
                braille(dots, w, h, &mut atlas.masks[i * len..(i + 1) * len]);
            }
        }
        atlas
    }

    /// Whether [`Self::for_char_set`] will produce visible ink for `c` (excluding `' '`).
    pub fn can_render_char(c: char) -> bool {
        if c == ' ' || c.is_control() {
            return false;
        }
        if is_kana(c) || braille_dots(c).is_some() {
            return true;
        }
        let Ok(face) = FontRef::new(FONT_DATA) else {
            return false;
        };
        let Some(id) = face.charmap().map(c) else {
            return false;
        };
        face.outline_glyphs().get(id).is_some()
    }

    /// Atlas for an arbitrary character list: rasterizes the bundled monospace
    /// font, narrows katakana from the kana font, and draws braille patterns
    /// (`U+2801..=U+28FF`) as crisp supersampled dots.
    pub fn for_char_set(chars: &[char], cell_w: u32, cell_h: u32, font_px: f64) -> Self {
        let (w, h) = (cell_w, cell_h);
        let mut atlas = Self::rasterize_chars(FONT_DATA, chars, w, h, font_px, false)
            .expect("bundled font parses");
        let len = (w * h) as usize;
        let kana: Vec<char> = chars
            .iter()
            .map(|&c| if is_kana(c) { c } else { ' ' })
            .collect();
        if kana.iter().any(|&c| c != ' ') {
            let k = Self::rasterize_chars(KANA_FONT, &kana, w, h, font_px, true)
                .expect("bundled kana font parses");
            for (i, _) in kana.iter().enumerate().filter(|(_, c)| **c != ' ') {
                atlas.masks[i * len..(i + 1) * len]
                    .copy_from_slice(&k.masks[i * len..(i + 1) * len]);
            }
        }
        for (i, &c) in chars.iter().enumerate() {
            if let Some(dots) = braille_dots(c) {
                braille(dots, w, h, &mut atlas.masks[i * len..(i + 1) * len]);
            }
        }
        atlas
    }

    /// Atlas from arbitrary font bytes.
    pub fn with_font(
        font: &[u8],
        cell_w: u32,
        cell_h: u32,
        font_px: f64,
    ) -> Result<Self, AtlasError> {
        Self::with_chars(font, &GLYPHS, cell_w, cell_h, font_px)
    }

    /// An atlas of these characters (mask `i` is `chars[i]`'s), for text
    /// beyond the scene's glyphs, such as a signature.
    pub fn with_chars(
        font: &[u8],
        chars: &[char],
        cell_w: u32,
        cell_h: u32,
        font_px: f64,
    ) -> Result<Self, AtlasError> {
        Self::rasterize_chars(font, chars, cell_w, cell_h, font_px, false)
    }

    /// With `narrow`, a glyph wider than the cell is narrowed to fit it.
    fn rasterize_chars(
        font: &[u8],
        chars: &[char],
        cell_w: u32,
        cell_h: u32,
        font_px: f64,
        narrow: bool,
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
        let mut masks = vec![0u8; cell_len * chars.len()];
        for (i, &ch) in chars.iter().enumerate() {
            if ch == ' ' {
                continue;
            }
            let Some(id) = charmap.map(ch) else {
                continue;
            };
            let Some(glyph) = outlines.get(id) else {
                continue;
            };
            let mut advance = f64::from(glyph_metrics.advance_width(id).unwrap_or(0.0)) * scale;
            let room = f64::from(cell_w) * 0.94;
            let sx = if narrow && advance > room {
                room / advance
            } else {
                1.0
            };
            advance *= sx;
            let mut outline = Flattener {
                segs: Vec::new(),
                ox: (f64::from(cell_w) - advance) * 0.5,
                baseline,
                scale,
                sx,
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
    /// How much narrower than the font draws it (1: as drawn).
    sx: f64,
    cur: (f64, f64),
    start: (f64, f64),
}

impl Flattener {
    fn map(&self, x: f32, y: f32) -> (f64, f64) {
        (
            self.ox + f64::from(x) * self.scale * self.sx,
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

/// Katakana, drawn from the kana font.
fn is_kana(c: char) -> bool {
    ('\u{30A0}'..='\u{30FF}').contains(&c)
}

/// A braille character's dots (bit 0 is dot 1), if it is one.
fn braille_dots(c: char) -> Option<u8> {
    let v = u32::from(c);
    (0x2801..=0x28FF).contains(&v).then(|| (v - 0x2800) as u8)
}

/// Draws braille dots as round dots: two columns of four, over the height
/// the letters take, the same at every size (supersampled, f64 only, so
/// the same on every machine).
fn braille(dots: u8, w: u32, h: u32, out: &mut [u8]) {
    // Dot n's column and row (Unicode's order: 1-3 and 7 down the left, 4-6
    // and 8 down the right).
    const PLACE: [(u32, u32); 8] = [
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 1),
        (1, 2),
        (0, 3),
        (1, 3),
    ];
    let (fw, fh) = (f64::from(w), f64::from(h));
    let r = (fw * 0.14).min(fh * 0.07);
    let centres: Vec<(f64, f64)> = PLACE
        .iter()
        .enumerate()
        .filter(|(n, _)| dots & (1 << n) != 0)
        .map(|(_, &(c, row))| {
            (
                fw * (0.3 + 0.4 * f64::from(c)),
                fh * (0.17 + 0.2 * f64::from(row)),
            )
        })
        .collect();
    let s = f64::from(SUPERSAMPLE);
    for y in 0..h {
        for x in 0..w {
            let mut hit = 0u32;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let px = f64::from(x) + (f64::from(sx) + 0.5) / s;
                    let py = f64::from(y) + (f64::from(sy) + 0.5) / s;
                    if centres
                        .iter()
                        .any(|&(cx, cy)| (px - cx) * (px - cx) + (py - cy) * (py - cy) <= r * r)
                    {
                        hit += 1;
                    }
                }
            }
            out[(y * w + x) as usize] =
                ((hit * 255 + SUPERSAMPLE * SUPERSAMPLE / 2) / (SUPERSAMPLE * SUPERSAMPLE)) as u8;
        }
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
    fn every_letter_set_has_ink_where_its_glyphs_do() {
        let spec = clairlune_scene::graph::grid_spec(1920, 1080);
        let text = GlyphAtlas::for_grid(&spec);
        assert_eq!(GlyphAtlas::for_letters(&spec, "text"), text);
        for set in clairlune_scene::glyphs::LETTERS {
            let a = GlyphAtlas::for_letters(&spec, set);
            for (g, c) in clairlune_scene::glyphs::letters(set).iter().enumerate() {
                let g = g as u8;
                assert_eq!(ink(&a, g) == 0, g == EMPTY, "{set}: {c:?} ({g})");
            }
        }
        // Braille is dots: one dot inks less than all eight.
        let b = GlyphAtlas::for_letters(&spec, "braille");
        assert!(ink(&b, id('.')) * 4 < ink(&b, id('@')));
    }

    #[test]
    fn atlas_is_deterministic() {
        let spec = grid_spec(640, 360);
        assert_eq!(GlyphAtlas::for_grid(&spec), GlyphAtlas::for_grid(&spec));
    }
}
