//! Frame compositing: the baked background plus animated elements at time `t`.
//! Particles snap to the cell grid (exact palette, small GIF diffs); sprites use
//! integer nearest-neighbour scaling.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::palette::{IndexedPalette, LEVELS, ink_level};
use clairlune_scene::{SceneGraph, SpriteInstance};

/// Composites one frame at `t_ms` over a baked background.
pub fn render_frame(
    background: &IndexedImage,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    t_ms: u64,
) -> IndexedImage {
    let mut img = background.clone();
    for (col, row, p) in graph.particles_at(t_ms) {
        draw_particle(&mut img, graph, atlas, col, row, p.glyph, p.level);
    }
    for sprite in graph.sprites_at(t_ms) {
        draw_sprite(&mut img, &sprite, palette);
    }
    img
}

/// A particle replaces its whole cell: background band, then the glyph in ember colour.
fn draw_particle(
    img: &mut IndexedImage,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    col: u32,
    row: u32,
    glyph: u8,
    level: u8,
) {
    let g = &graph.grid;
    let shade =
        ((u32::from(level.min(LEVELS)) * 255 + u32::from(LEVELS) / 2) / u32::from(LEVELS)) as u8;
    let mask = atlas.mask(glyph);
    let (x0, y0) = (g.origin_x + col * g.cell_w, g.origin_y + row * g.cell_h);
    let w = img.width as usize;
    for my in 0..g.cell_h {
        let y = y0 + my;
        let rt = graph.lighting.row_terms(y);
        for mx in 0..g.cell_w {
            let x = x0 + mx;
            let band = graph.lighting.band_in_row(&rt, x);
            let lvl = ink_level(mask[(my * g.cell_w + mx) as usize], shade);
            img.pixels[y as usize * w + x as usize] = if lvl > 0 {
                IndexedPalette::ember(band, lvl)
            } else {
                band
            };
        }
    }
}

fn draw_sprite(img: &mut IndexedImage, s: &SpriteInstance, palette: &IndexedPalette) {
    let map = s.kind.map();
    let (w, h) = (img.width as i32, img.height as i32);
    let scale = s.scale as i32;
    for my in 0..map.height {
        for mx in 0..map.width {
            let sx = if s.mirror { map.width - 1 - mx } else { mx };
            let Some(rgb) = map.pixel(sx, my) else {
                continue;
            };
            let idx = palette.sprite_index(rgb).expect("sprite colour in palette");
            let px = s.x + mx as i32 * scale;
            let py = s.y + my as i32 * scale;
            for y in py.max(0)..(py + scale).min(h) {
                let line = (y * w) as usize;
                for x in px.max(0)..(px + scale).min(w) {
                    img.pixels[line + x as usize] = idx;
                }
            }
        }
    }
}
