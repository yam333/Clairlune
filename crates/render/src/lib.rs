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

pub mod art;
pub mod atlas;
pub mod bake;
pub mod clock;
pub mod encode;
pub mod frame;
pub mod gif_export;
pub mod journey;
pub mod package;
pub mod palette;
mod projectile;
mod tether;

pub use art::{BuiltinArt, OwnedPixmap, Pixmap, SceneArt};
pub use atlas::GlyphAtlas;
pub use bake::bake_static;
pub use clock::Clock;
pub use encode::{encode_png, save_png, to_rgba};
pub use frame::{
    IncrementalFrame, PixelRect, hero_time, hero_time_with_art, render_frame, render_frame_clocked,
    render_frame_with_art,
};
pub use gif_export::{
    GifError, GifStats, Smoothness, encode_gif, encode_package_gif, frame_delays, valid_loop_ms,
};
pub use journey::JourneyBake;
pub use package::{PACKAGE_FORMAT, PackageError, ScenePackage};
pub use palette::IndexedPalette;

use clairlune_scene::activity::builtin_catalog;
use clairlune_scene::{BuildError, BuildOptions, SceneGraph, SceneRecipe, build};
use clairlune_sprites::{Character, builtin_character};
use std::time::Instant;

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
    let started = Instant::now();
    let graph = build(recipe, opts)?;
    if opts.buddy
        && graph.character.is_some()
        && let Some(a) = graph
            .activity
            .as_deref()
            .and_then(|id| builtin_catalog().get(id))
        && builtin_character()
            .animation(&a.id, &a.animation.start)
            .is_some_and(|an| an.name == "placeholder")
    {
        tracing::warn!(target: "clairlune_render", activity = %a.id, "no drawn art for this activity; drawing the placeholder character");
    }
    let atlas = GlyphAtlas::for_grid(&graph.grid);
    let palette = IndexedPalette::for_graph(&graph);
    let mut background = bake_static(&graph, &atlas);
    let t = t_ms.unwrap_or_else(|| hero_time(&graph));
    if let Some(j) = JourneyBake::new(&graph, &atlas) {
        background = j.background_at(&graph, &background, t);
    }
    let image = render_frame(&background, &graph, &atlas, &palette, t);
    tracing::info!(
        target: "clairlune_render",
        seed = recipe.seed,
        recipe_hash = %recipe.hash_hex(),
        width = opts.width,
        height = opts.height,
        theme = opts.theme.as_str(),
        buddy = opts.buddy,
        t_ms = t,
        palette = palette.len(),
        elapsed_ms = started.elapsed().as_secs_f64() * 1e3,
        "still rendered"
    );
    Ok(Still {
        image,
        palette,
        graph,
        t_ms: t,
    })
}
