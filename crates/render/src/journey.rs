//! Journeys (docs/scrolling-scenes.md): the layers that scroll past, baked
//! once into strips and laid over the still background at each frame's
//! offsets. A strip holds ink levels, not colours: ink takes its colour
//! from the background band under it (vignette, glow), and the bands stay
//! where they are on screen while the land moves.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::palette::{IndexedPalette, ink_level};
use clairlune_scene::glyphs::EMPTY;
use clairlune_scene::{Cell, SceneGraph};

/// A strip pixel that lets what is behind show through.
const CLEAR: u8 = u8::MAX;

/// One layer as pixels: `CLEAR`, 0 (the band: hides what is behind) or an
/// ink level, for the rows from `y0` down.
#[derive(Clone, Debug, Default)]
struct LevelStrip {
    /// Width in pixels: the period.
    period: usize,
    y0: usize,
    rows: usize,
    levels: Vec<u8>,
}

/// A journey's moving layers, ready to lay over the still background.
#[derive(Clone, Debug)]
pub struct JourneyBake {
    strips: Vec<LevelStrip>,
    /// The background bands of the rows from `y0` down (what a strip's
    /// ink is coloured by).
    bands: Vec<u8>,
    y0: usize,
}

impl JourneyBake {
    /// `None` for a scene that stays put.
    pub fn new(graph: &SceneGraph, atlas: &GlyphAtlas) -> Option<Self> {
        let journey = graph.journey.as_ref()?;
        let g = &graph.grid;
        let (cw, ch) = (g.cell_w as usize, g.cell_h as usize);
        let mut strips = Vec::with_capacity(journey.layers.len());
        for layer in &journey.layers {
            let lc = layer.cols as usize;
            let cell = |col: usize, row: usize| layer.cells[row * lc + col];
            let Some(first) =
                (0..g.rows as usize).find(|&r| (0..lc).any(|c| cell(c, r) != Cell::default()))
            else {
                strips.push(LevelStrip::default());
                continue;
            };
            let period = lc * cw;
            let rows = (g.rows as usize - first) * ch;
            let mut levels = vec![CLEAR; period * rows];
            for row in first..g.rows as usize {
                for col in 0..lc {
                    let c = cell(col, row);
                    if c == Cell::default() {
                        continue;
                    }
                    let mask = atlas.mask(c.glyph);
                    let (x0, y0) = (col * cw, (row - first) * ch);
                    for my in 0..ch {
                        let line = &mut levels[(y0 + my) * period + x0..][..cw];
                        for (p, &a) in line.iter_mut().zip(&mask[my * cw..(my + 1) * cw]) {
                            *p = if c.glyph == EMPTY || a == 0 {
                                0
                            } else {
                                ink_level(a, c.shade)
                            };
                        }
                    }
                }
            }
            strips.push(LevelStrip {
                period,
                y0: g.origin_y as usize + first * ch,
                rows,
                levels,
            });
        }
        let (w, h) = (graph.width as usize, graph.height as usize);
        let y0 = strips
            .iter()
            .filter(|s| s.rows > 0)
            .map(|s| s.y0)
            .min()
            .unwrap_or(h);
        let mut bands = vec![0u8; (h - y0) * w];
        for (i, row) in bands.chunks_exact_mut(w).enumerate() {
            graph.lighting.fill_row((y0 + i) as u32, row);
        }
        Some(JourneyBake { strips, bands, y0 })
    }

    /// The background at `t_ms` into `out`: the still background, then each
    /// strip from its offset, back to front.
    pub fn draw(
        &self,
        graph: &SceneGraph,
        still: &IndexedImage,
        t_ms: u64,
        out: &mut IndexedImage,
    ) {
        out.pixels.copy_from_slice(&still.pixels);
        let Some(journey) = graph.journey.as_ref() else {
            return;
        };
        let g = &graph.grid;
        let w = graph.width as usize;
        let (gx, gw) = (g.origin_x as usize, (g.cols * g.cell_w) as usize);
        for (i, s) in self.strips.iter().enumerate() {
            if s.rows == 0 || s.period < gw {
                continue;
            }
            let off = journey.offset_px(i, g.cell_w, t_ms) as usize;
            // The screen shows the strip from `off`, wrapping round once.
            let first = (s.period - off).min(gw);
            for r in 0..s.rows {
                let y = s.y0 + r;
                if y >= graph.height as usize {
                    break;
                }
                let src = &s.levels[r * s.period..(r + 1) * s.period];
                let band = &self.bands[(y - self.y0) * w + gx..][..gw];
                let dst = &mut out.pixels[y * w + gx..][..gw];
                lay(&mut dst[..first], &src[off..off + first], &band[..first]);
                if first < gw {
                    lay(&mut dst[first..], &src[..gw - first], &band[first..]);
                }
            }
        }
    }

    /// [`JourneyBake::draw`] into a new image.
    pub fn background_at(
        &self,
        graph: &SceneGraph,
        still: &IndexedImage,
        t_ms: u64,
    ) -> IndexedImage {
        let mut out = still.clone();
        self.draw(graph, still, t_ms, &mut out);
        out
    }
}

/// One run of a strip over the frame.
fn lay(dst: &mut [u8], src: &[u8], band: &[u8]) {
    for ((d, &v), &b) in dst.iter_mut().zip(src).zip(band) {
        if v != CLEAR {
            *d = if v == 0 { b } else { IndexedPalette::ink(b, v) };
        }
    }
}
