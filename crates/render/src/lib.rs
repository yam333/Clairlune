//! Rendering for Clairlune: glyph atlas, indexed palette, static bake, frame
//! compositing and PNG export.
//!
//! Every pixel is a palette index. Colours are looked up, never blended at
//! render time, so a scene has at most 255 colours (index 255 is reserved for GIF
//! transparency) and the still PNG, GIF frames and the live renderer can match
//! exactly (PLAN.md section 4.5).

/// Compiles the README example as a doctest so it can't go stale.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod atlas;
pub mod bake;
pub mod encode;
pub mod frame;
pub mod palette;

pub use atlas::GlyphAtlas;
pub use bake::bake_static;
pub use encode::{encode_png, save_png, to_rgba};
pub use frame::{hero_time, render_frame};
pub use palette::IndexedPalette;

use clairlune_scene::{BuildError, BuildOptions, SceneGraph, SceneRecipe, build};

/// An image of palette indices, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl IndexedImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize],
        }
    }
}

/// A rendered still frame with everything needed to encode or inspect it.
#[derive(Clone, Debug)]
pub struct Still {
    pub image: IndexedImage,
    pub palette: IndexedPalette,
    pub graph: SceneGraph,
    /// The animation time that was rendered.
    pub t_ms: u64,
}

/// Renders a recipe to a still frame at `t_ms`, or at the hero time when `None`.
pub fn render_still(
    recipe: &SceneRecipe,
    opts: &BuildOptions,
    t_ms: Option<u64>,
) -> Result<Still, BuildError> {
    let graph = build(recipe, opts)?;
    let atlas = GlyphAtlas::for_grid(&graph.grid);
    let palette = IndexedPalette::new(&graph.family);
    let background = bake_static(&graph, &atlas);
    let t = t_ms.unwrap_or_else(|| hero_time(&graph));
    let image = render_frame(&background, &graph, &atlas, &palette, t);
    Ok(Still {
        image,
        palette,
        graph,
        t_ms: t,
    })
}
