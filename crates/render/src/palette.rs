//! The indexed palette of one rendering (at most 255 colours).
//!
//! Layout (by construction, so every pixel lands on an entry):
//!
//! | Indices | Entries |
//! |---|---|
//! | 0-7 | background bands (vignette 0-3, warm glow 4-7) |
//! | 8-55 | ink: band x 6 levels (ink in glow bands is tinted toward the accent) |
//! | 56-103 | ember particles: band x 6 levels |
//! | 104.. | sprite colours |
//! | 255 | reserved (GIF transparency) |

use clairlune_scene::graph::BANDS;
use clairlune_scene::graph::SpriteKind;
use clairlune_scene::{PaletteFamily, Rgb};

/// Ink levels per band (glyph alpha x shade, quantised).
pub const LEVELS: u8 = 6;
pub const INK_BASE: u8 = BANDS;
pub const EMBER_BASE: u8 = INK_BASE + BANDS * LEVELS;
pub const SPRITE_BASE: u8 = EMBER_BASE + BANDS * LEVELS;
/// Reserved for GIF transparency; never used by a rendered pixel.
pub const TRANSPARENT_INDEX: u8 = 255;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPalette {
    colors: Vec<Rgb>,
}

impl IndexedPalette {
    /// A palette from stored colours (scene packages), in the layout `new`
    /// produces.
    pub fn from_colors(colors: Vec<Rgb>) -> Self {
        IndexedPalette { colors }
    }

    /// The palette a scene graph renders with: generator 4 and later give
    /// light scenes a stronger ink ramp (below); everything else keeps the
    /// original, linear one.
    pub fn for_graph(graph: &clairlune_scene::SceneGraph) -> Self {
        let strong = graph.generator >= 4 && graph.family.theme == clairlune_scene::Theme::Light;
        Self::with_ramp(&graph.family, strong)
    }

    /// The original palette: ink levels blend linearly from background to
    /// ink.
    pub fn new(family: &PaletteFamily) -> Self {
        Self::with_ramp(family, false)
    }

    /// With `strong`, ink levels follow `1 - (1 - l/6)^1.8`: thin strokes and
    /// faint far layers come out about twice as dark, so ASCII on a light
    /// background reads as clearly as light strokes on a dark one. Full ink
    /// stays full ink.
    fn with_ramp(family: &PaletteFamily, strong: bool) -> Self {
        let level = |l: u8| -> f64 {
            let x = f64::from(l) / f64::from(LEVELS);
            if strong {
                1.0 - clairlune_scene::math::pow(1.0 - x, 1.8)
            } else {
                x
            }
        };
        let mut colors = Vec::with_capacity(128);
        let band = |b: u8| -> Rgb {
            if b < 4 {
                family.bg_edge.mix(family.bg_center, f64::from(b) / 3.0)
            } else {
                let t = family.glow_strength * f64::from(b - 3) / 4.0;
                family.bg_center.mix(family.accent, t)
            }
        };
        for b in 0..BANDS {
            colors.push(band(b));
        }
        for b in 0..BANDS {
            let ink = if b < 4 {
                family.ink
            } else {
                family
                    .ink
                    .mix(family.accent, family.tint_strength * f64::from(b - 3) / 4.0)
            };
            for l in 1..=LEVELS {
                colors.push(band(b).mix(ink, level(l)));
            }
        }
        for b in 0..BANDS {
            for l in 1..=LEVELS {
                colors.push(band(b).mix(family.ember, f64::from(l) / f64::from(LEVELS)));
            }
        }
        debug_assert_eq!(colors.len(), SPRITE_BASE as usize);
        for kind in SpriteKind::ALL {
            for rgb in kind.map().palette() {
                if !colors[SPRITE_BASE as usize..].contains(&rgb) {
                    colors.push(rgb);
                }
            }
        }
        for rgb in clairlune_sprites::builtin_character().colors() {
            if !colors[SPRITE_BASE as usize..].contains(&rgb) {
                colors.push(rgb);
            }
        }
        assert!(
            colors.len() <= TRANSPARENT_INDEX as usize,
            "palette over budget"
        );
        Self { colors }
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    pub fn colors(&self) -> &[Rgb] {
        &self.colors
    }

    /// `r g b` triplets for a PNG/GIF palette chunk.
    pub fn rgb_bytes(&self) -> Vec<u8> {
        self.colors.iter().flat_map(|c| [c.r, c.g, c.b]).collect()
    }

    /// Ink entry for a background band and level `1..=LEVELS`.
    #[inline]
    pub fn ink(band: u8, level: u8) -> u8 {
        INK_BASE + band * LEVELS + level - 1
    }

    /// Ember entry for a background band and level `1..=LEVELS`.
    #[inline]
    pub fn ember(band: u8, level: u8) -> u8 {
        EMBER_BASE + band * LEVELS + level - 1
    }

    /// Adds sprite colours it lacks (a character drawn with colours the
    /// built-in art doesn't use), as far as the budget allows.
    pub fn with_sprite_colors(mut self, extra: impl IntoIterator<Item = Rgb>) -> Self {
        for rgb in extra {
            if self.colors.len() >= TRANSPARENT_INDEX as usize {
                break;
            }
            if self.sprite_index(rgb).is_none() {
                self.colors.push(rgb);
            }
        }
        self
    }

    /// Palette index of a sprite colour.
    pub fn sprite_index(&self, rgb: Rgb) -> Option<u8> {
        self.colors[SPRITE_BASE as usize..]
            .iter()
            .position(|&c| c == rgb)
            .map(|i| SPRITE_BASE + i as u8)
    }
}

/// Ink level `0..=LEVELS` from glyph alpha and cell shade (both 0-255).
#[inline]
pub fn ink_level(alpha: u8, shade: u8) -> u8 {
    let v = u32::from(alpha) * u32::from(shade) * u32::from(LEVELS);
    ((v + 65025 / 2) / 65025) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_scene::palette::FAMILIES;

    #[test]
    fn every_family_fits_the_budget() {
        for f in &FAMILIES {
            let p = IndexedPalette::new(f);
            assert!(p.len() <= 255, "{}: {}", f.id, p.len());
            assert!(p.sprite_index(Rgb::hex(0xD9785C)).is_some());
        }
    }

    #[test]
    fn ink_level_range() {
        assert_eq!(ink_level(0, 255), 0);
        assert_eq!(ink_level(255, 255), LEVELS);
        assert_eq!(ink_level(255, 0), 0);
    }
}
