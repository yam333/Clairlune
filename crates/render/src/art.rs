//! Everything a frame draws besides the background: the activity's state
//! machine, the character's frames and the props' pixel art. Live scenes take
//! them from the built-in catalog and art ([`BuiltinArt`]); saved scenes carry
//! their own copy (`crate::package`), so they play the same after the
//! built-in art or activities change.

use clairlune_scene::activity::{Activity, builtin_catalog};
use clairlune_scene::placeholder::PixelMap;
use clairlune_scene::{Rgb, SceneGraph, SpriteKind};
use clairlune_sprites::{Character, builtin_character};

/// A small pixel image with transparency (prop art).
pub trait Pixmap {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    /// Colour of pixel `(x, y)`, or `None` when transparent.
    fn pixel(&self, x: u32, y: u32) -> Option<Rgb>;
}

impl Pixmap for PixelMap {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn pixel(&self, x: u32, y: u32) -> Option<Rgb> {
        PixelMap::pixel(self, x, y)
    }
}

/// An owned pixel image (props copied into a saved scene).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedPixmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Option<Rgb>>,
}

impl OwnedPixmap {
    pub fn copy_of(p: &dyn Pixmap) -> Self {
        let pixels = (0..p.height())
            .flat_map(|y| (0..p.width()).map(move |x| (x, y)))
            .map(|(x, y)| p.pixel(x, y))
            .collect();
        OwnedPixmap {
            width: p.width(),
            height: p.height(),
            pixels,
        }
    }
}

impl Pixmap for OwnedPixmap {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn pixel(&self, x: u32, y: u32) -> Option<Rgb> {
        self.pixels[(y * self.width + x) as usize]
    }
}

/// The art a scene is drawn with.
pub trait SceneArt {
    /// The scene's activity (state machine, effects), if it has one.
    fn activity(&self) -> Option<&Activity>;
    fn character(&self) -> &dyn Character;
    /// Pixel art for a prop.
    fn prop(&self, kind: SpriteKind) -> Option<&dyn Pixmap>;
}

/// The built-in catalog and art, for scenes made by this generator.
pub struct BuiltinArt {
    activity: Option<&'static Activity>,
}

impl BuiltinArt {
    pub fn for_graph(graph: &SceneGraph) -> Self {
        BuiltinArt {
            activity: graph
                .activity
                .as_deref()
                .and_then(|id| builtin_catalog().get(id)),
        }
    }
}

impl SceneArt for BuiltinArt {
    fn activity(&self) -> Option<&Activity> {
        self.activity
    }
    fn character(&self) -> &dyn Character {
        builtin_character()
    }
    fn prop(&self, kind: SpriteKind) -> Option<&dyn Pixmap> {
        Some(kind.map())
    }
}
