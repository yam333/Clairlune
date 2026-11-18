//! Static bake: background bands plus every glyph cell, rendered once per scene.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::palette::{IndexedPalette, ink_level};
use clairlune_scene::SceneGraph;
use clairlune_scene::glyphs::EMPTY;

/// Renders the static background of a scene graph into palette indices.
pub fn bake_static(graph: &SceneGraph, atlas: &GlyphAtlas) -> IndexedImage {
    let mut img = IndexedImage::new(graph.width, graph.height);
    let w = graph.width as usize;
    for (y, row) in img.pixels.chunks_exact_mut(w).enumerate() {
        graph.lighting.fill_row(y as u32, row);
    }

    let g = &graph.grid;
    let (cw, ch) = (g.cell_w as usize, g.cell_h as usize);
    for row in 0..g.rows {
        for col in 0..g.cols {
            let cell = graph.cell(col, row);
            if cell.glyph == EMPTY || cell.shade == 0 {
                continue;
            }
            let mask = atlas.mask(cell.glyph);
            let x0 = g.origin_x as usize + col as usize * cw;
            let y0 = g.origin_y as usize + row as usize * ch;
            for my in 0..ch {
                let line = (y0 + my) * w + x0;
                let dst = &mut img.pixels[line..line + cw];
                let src = &mask[my * cw..(my + 1) * cw];
                for (p, &a) in dst.iter_mut().zip(src) {
                    if a == 0 {
                        continue;
                    }
                    let level = ink_level(a, cell.shade);
                    if level > 0 {
                        // Cells never overlap, so `*p` is still the background band.
                        *p = IndexedPalette::ink(*p, level);
                    }
                }
            }
        }
    }
    img
}
