//! Clairlune's live wallpaper (PLAN.md sections 2 and 5.3): a saved scene
//! plays behind the desktop icons.
//!
//! [`Player`] keeps the frame between ticks with the incremental compositor
//! and writes only the rectangles that changed into a 0RGB buffer, which the
//! presenter copies to the screen as damage. The pixels come from the same
//! compositor as the still PNG, so the first live frame (at the hero time)
//! is the static wallpaper exactly, and so is every frame's palette.

use clairlune_render::clock::Clock;
use clairlune_render::{IncrementalFrame, IndexedImage, PixelRect, ScenePackage};
use clairlune_scene::Theme;
use std::sync::Arc;
use std::time::Instant;

#[cfg(windows)]
pub mod d3d11;
#[cfg(windows)]
pub mod desktop;
pub mod gpu;

/// Plays one saved scene, one frame at a time.
pub struct Player {
    pkg: Arc<ScenePackage>,
    theme: Theme,
    buddy: bool,
    frame: IncrementalFrame,
    /// Palette index to 0RGB.
    colors: [u32; 256],
    /// Scene time when playback started (the hero time, so the first frame
    /// is the still), and when that was.
    t0: u64,
    started: Instant,
}

impl Player {
    /// `None` if the scene has no such theme.
    pub fn new(pkg: Arc<ScenePackage>, theme: Theme, buddy: bool) -> Option<Player> {
        let frame = pkg.incremental(theme)?;
        let mut colors = [0u32; 256];
        for (i, c) in pkg.palette(theme)?.colors().iter().enumerate() {
            colors[i] = (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b);
        }
        let t0 = pkg.hero_ms(theme)?;
        Some(Player {
            pkg,
            theme,
            buddy,
            frame,
            colors,
            t0,
            started: Instant::now(),
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.pkg.width, self.pkg.height)
    }

    /// The current frame as palette indices.
    pub fn image(&self) -> &IndexedImage {
        self.frame.image()
    }

    /// The palette as 0RGB, by index.
    pub fn colors(&self) -> &[u32; 256] {
        &self.colors
    }

    /// Scene time now: the hero time plus the time since playback started.
    /// A pause only stops drawing; the next frame shows the time it is.
    pub fn now_ms(&self) -> u64 {
        self.t0 + self.started.elapsed().as_millis() as u64
    }

    /// Moves to `t_ms`; returns the rectangles that changed.
    pub fn advance(&mut self, t_ms: u64) -> Vec<PixelRect> {
        self.pkg
            .advance(&mut self.frame, self.theme, self.buddy, t_ms, Clock::Live)
            .unwrap_or_default()
    }

    /// Writes those rectangles of the current frame into a 0RGB buffer of the
    /// frame's size.
    pub fn write(&self, buf: &mut [u32], rects: &[PixelRect]) {
        let img = self.frame.image();
        let w = img.width as usize;
        debug_assert_eq!(buf.len(), img.pixels.len());
        for r in rects {
            for y in r.y..r.y + r.h {
                let a = y as usize * w + r.x as usize;
                let b = a + r.w as usize;
                for (out, &i) in buf[a..b].iter_mut().zip(&img.pixels[a..b]) {
                    *out = self.colors[usize::from(i)];
                }
            }
        }
    }

    /// One rectangle of the current frame as 0RGB pixels, row by row.
    pub fn rect_pixels(&self, r: &PixelRect, out: &mut Vec<u32>) {
        let img = self.frame.image();
        let w = img.width as usize;
        out.clear();
        for y in r.y..r.y + r.h {
            let a = y as usize * w + r.x as usize;
            out.extend(
                img.pixels[a..a + r.w as usize]
                    .iter()
                    .map(|&i| self.colors[usize::from(i)]),
            );
        }
    }

    /// Writes the whole current frame.
    pub fn write_all(&self, buf: &mut [u32]) {
        for (out, &i) in buf.iter_mut().zip(&self.frame.image().pixels) {
            *out = self.colors[usize::from(i)];
        }
    }
}

/// Live frame rates, by their names in Settings (PLAN.md section 5.3).
pub const SMOOTHNESS_FPS: [(u32, &str); 5] = [
    (6, "Battery saver"),
    (12, "Balanced"),
    (24, "Smooth"),
    (30, "Very smooth"),
    (60, "Ultra smooth"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_render::to_rgba;
    use clairlune_scene::{Aspect, ContentContext, compose};

    fn player() -> Player {
        let r = compose(42, Aspect::from_size(320, 180), &ContentContext::default());
        let pkg = Arc::new(ScenePackage::capture(&r, 320, 180).unwrap());
        let theme = pkg.themes()[0];
        Player::new(pkg, theme, true).unwrap()
    }

    fn rgb(buf: &[u32]) -> Vec<u8> {
        buf.iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255])
            .collect()
    }

    #[test]
    fn the_first_frame_is_the_still_and_updates_keep_the_buffer_exact() {
        let mut p = player();
        let (w, h) = p.size();
        let mut buf = vec![0u32; (w * h) as usize];
        let t0 = p.t0;
        let rects = p.advance(t0);
        p.write_all(&mut buf);
        let _ = rects;
        let (still, palette) = p.pkg.still(p.theme, true).unwrap();
        assert_eq!(rgb(&buf), to_rgba(&still, palette));
        // Then only the changed rects, as a presenter does.
        for k in 1..40u64 {
            let t = t0 + k * 83;
            let rects = p.advance(t);
            p.write(&mut buf, &rects);
            let (full, palette) = p.pkg.frame(p.theme, true, t, Clock::Live).unwrap();
            assert_eq!(rgb(&buf), to_rgba(&full, palette), "t {t}");
        }
    }
}
