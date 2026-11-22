//! A signature in the corner of a saved image, written into the scene's
//! grid the way the scene writes its own text: each cell it takes is cleared
//! to the background under it, then inked in that background's band, so it
//! sits in the picture rather than on top of it.

use crate::IndexedImage;
use crate::atlas::{FONT_DATA, GlyphAtlas};
use crate::palette::{EMBER_BASE, INK_BASE, IndexedPalette, LEVELS, SPRITE_BASE};
use clairlune_scene::grid_spec;

/// What saved images are signed with.
pub const SIGNATURE: &str = "clairlune \u{b7} by yam333";

/// The signature's strongest ink level (of [`LEVELS`]): quieter than the
/// scene's own text.
const STRENGTH: u32 = 4;

/// A signature laid out for images of one size.
#[derive(Clone, Debug)]
pub struct Signature {
    atlas: GlyphAtlas,
    chars: u32,
    /// The first cell's top-left pixel.
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Signature {
    /// `text` in the bottom-right corner, one cell in from the edges, at
    /// the scene's own cell size; `None` when the image is too narrow to
    /// hold it in under half its width.
    pub fn new(width: u32, height: u32, text: &str) -> Option<Signature> {
        let spec = grid_spec(width, height);
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len() as u32;
        if n == 0 || n > 200 || spec.cols < 2 * (n + 3) || spec.rows < 6 {
            return None;
        }
        let atlas =
            GlyphAtlas::with_chars(FONT_DATA, &chars, spec.cell_w, spec.cell_h, spec.font_px)
                .ok()?;
        // A cell of margin and a cell of clear space on the right.
        let col = spec.cols - n - 2;
        let row = spec.rows - 2;
        Some(Signature {
            atlas,
            chars: n,
            x: spec.origin_x + col * spec.cell_w,
            y: spec.origin_y + row * spec.cell_h,
            width,
            height,
        })
    }

    /// Writes it into an image of the size it was laid out for (others are
    /// left alone).
    pub fn apply(&self, img: &mut IndexedImage) {
        if (img.width, img.height) != (self.width, self.height) {
            return;
        }
        let (cw, ch) = (self.atlas.cell_w, self.atlas.cell_h);
        let w = img.width as usize;
        // Its cells and one either side, cleared to the background.
        let x0 = self.x.saturating_sub(cw);
        let x1 = (self.x + (self.chars + 1) * cw).min(img.width);
        let y1 = (self.y + ch).min(img.height);
        for py in self.y..y1 {
            for px in x0..x1 {
                let i = py as usize * w + px as usize;
                img.pixels[i] = band_of(img.pixels[i]);
            }
        }
        for c in 0..self.chars {
            let cx = self.x + c * cw;
            for (k, &a) in self.atlas.mask(c as u8).iter().enumerate() {
                let level = ((u32::from(a) * STRENGTH + 127) / 255) as u8;
                let (px, py) = (cx + k as u32 % cw, self.y + k as u32 / cw);
                if level == 0 || px >= img.width || py >= img.height {
                    continue;
                }
                let i = py as usize * w + px as usize;
                img.pixels[i] = IndexedPalette::ink(img.pixels[i], level);
            }
        }
    }
}

/// The background band a palette entry is drawn over (sprite colours: the
/// edge's).
fn band_of(index: u8) -> u8 {
    if index < INK_BASE {
        index
    } else if index < EMBER_BASE {
        (index - INK_BASE) / LEVELS
    } else if index < SPRITE_BASE {
        (index - EMBER_BASE) / LEVELS
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(w: u32, h: u32, index: u8) -> IndexedImage {
        IndexedImage {
            width: w,
            height: h,
            pixels: vec![index; (w * h) as usize],
        }
    }

    #[test]
    fn signs_the_bottom_right_in_the_backgrounds_own_ink() {
        let mut img = filled(1920, 1080, IndexedPalette::ink(5, 6));
        let sig = Signature::new(1920, 1080, SIGNATURE).expect("room for it");
        sig.apply(&mut img);
        let (w, h) = (1920usize, 1080usize);
        let changed: Vec<usize> = (0..w * h)
            .filter(|&i| img.pixels[i] != IndexedPalette::ink(5, 6))
            .collect();
        assert!(!changed.is_empty());
        for &i in &changed {
            let (x, y) = (i % w, i / w);
            // Only in the bottom-right corner...
            assert!(x > w / 2 && y > h * 9 / 10, "({x}, {y})");
            // ...as the band's background or its ink, never stronger.
            let p = img.pixels[i];
            assert!(
                p == 5
                    || (IndexedPalette::ink(5, 1)..=IndexedPalette::ink(5, STRENGTH as u8))
                        .contains(&p),
                "{p}"
            );
        }
        // Some of it is inked: the text is there.
        assert!(changed.iter().any(|&i| img.pixels[i] != 5));
    }

    #[test]
    fn small_images_go_unsigned() {
        assert!(Signature::new(160, 90, SIGNATURE).is_none());
        // A frame of another size is left alone.
        let sig = Signature::new(1920, 1080, SIGNATURE).unwrap();
        let mut other = filled(1280, 720, 2);
        sig.apply(&mut other);
        assert!(other.pixels.iter().all(|&p| p == 2));
    }

    #[test]
    fn every_entry_has_a_band() {
        assert_eq!(band_of(3), 3);
        assert_eq!(band_of(IndexedPalette::ink(6, 2)), 6);
        assert_eq!(band_of(IndexedPalette::ember(1, 6)), 1);
        assert_eq!(band_of(SPRITE_BASE + 3), 0);
    }
}
