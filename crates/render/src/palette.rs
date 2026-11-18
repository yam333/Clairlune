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

const SPRITE_KINDS: [SpriteKind; 3] = [
    SpriteKind::Character,
    SpriteKind::Campfire,
    SpriteKind::LogSeat,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPalette {
    colors: Vec<Rgb>,
}

impl IndexedPalette {
    pub fn new(family: &PaletteFamily) -> Self {
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
                colors.push(band(b).mix(ink, f64::from(l) / f64::from(LEVELS)));
            }
        }
        for b in 0..BANDS {
            for l in 1..=LEVELS {
                colors.push(band(b).mix(family.ember, f64::from(l) / f64::from(LEVELS)));
            }
        }
        debug_assert_eq!(colors.len(), SPRITE_BASE as usize);
        for kind in SPRITE_KINDS {
            for rgb in kind.map().palette() {
                if !colors[SPRITE_BASE as usize..].contains(&rgb) {
                    colors.push(rgb);
                }
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
