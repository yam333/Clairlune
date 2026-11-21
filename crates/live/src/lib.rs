//! Clairlune's live wallpaper (PLAN.md sections 2 and 5.3): a saved scene
//! plays behind the desktop icons.
//!
//! [`Player`] keeps the frame between ticks with the incremental compositor
//! and writes only the rectangles that changed into a 0RGB buffer, which the
//! presenter copies to the screen as damage. The pixels come from the same
//! compositor as the still PNG, with the same palette, so the first live
//! frame (at the hero time) is the static wallpaper, except that on a GPU
//! the particles sit at their own sub-cell positions ([`Player::set_smooth`])
//! instead of the cell grid.

use clairlune_render::clock::Clock;
use clairlune_render::{IncrementalFrame, IndexedImage, PixelRect, ScenePackage};
use clairlune_scene::Theme;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(windows)]
pub mod d3d11;
#[cfg(windows)]
pub mod desktop;
pub mod gpu;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod mac;

/// What a maximized or full-screen app pauses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoverPolicy {
    /// The monitor it covers.
    #[default]
    ThatMonitor,
    /// Every monitor.
    AllMonitors,
    /// Nothing.
    Off,
}

/// What one frame cost, for budgets and logs.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    /// Pixels uploaded or copied, over all monitors.
    pub pixels: u64,
    /// Rectangles uploaded or copied.
    pub rects: usize,
    /// Monitors paused because a window covers them.
    pub covered: usize,
    /// Paused everywhere: the session is locked or the displays are off.
    pub away: bool,
    /// Presents that waited over 50 ms for the swapchain (the compositor
    /// holding frames back).
    pub stalls: usize,
    /// Time spent compositing, uploading and presenting.
    pub busy: Duration,
    /// Of which uploading, and drawing plus presenting.
    pub upload: Duration,
    pub present: Duration,
}

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
    /// `Clock::Live` (particles on the cell grid) or `Clock::Smooth`.
    clock: Clock,
    /// The part of the frame this player shows (one monitor of a scene
    /// spread over several); the whole frame otherwise.
    view: PixelRect,
}

impl Player {
    /// `None` if the scene has no such theme.
    pub fn new(pkg: Arc<ScenePackage>, theme: Theme, buddy: bool) -> Option<Player> {
        let view = PixelRect {
            x: 0,
            y: 0,
            w: pkg.width,
            h: pkg.height,
        };
        Self::new_view(pkg, theme, buddy, view)
    }

    /// A player showing only `view` of the scene's frame (span mode: each
    /// monitor shows its part of one scene). Every method then works in the
    /// view's own coordinates. `None` if the scene has no such theme or the
    /// view isn't inside the frame.
    pub fn new_view(
        pkg: Arc<ScenePackage>,
        theme: Theme,
        buddy: bool,
        view: PixelRect,
    ) -> Option<Player> {
        let inside =
            |at: u32, len: u32, room: u32| at.checked_add(len).is_some_and(|end| end <= room);
        if view.w == 0
            || view.h == 0
            || !inside(view.x, view.w, pkg.width)
            || !inside(view.y, view.h, pkg.height)
        {
            return None;
        }
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
            clock: Clock::Live,
            view,
        })
    }

    /// Particles at sub-cell positions (`Clock::Smooth`), for presenters on
    /// a GPU: a higher frame rate then moves them smoothly. Off by default,
    /// so the first frame is the still exactly; the CPU fallback keeps it
    /// off (PLAN.md section 4.4).
    pub fn set_smooth(&mut self, on: bool) {
        self.clock = if on { Clock::Smooth } else { Clock::Live };
    }

    /// Another player of the same scene at the same moment, showing `view`
    /// of it (one scene on several screens, each showing its part). `None`
    /// if the view isn't inside the frame.
    pub fn with_view(&self, view: PixelRect) -> Option<Player> {
        let mut p = Player::new_view(self.pkg.clone(), self.theme, self.buddy, view)?;
        p.t0 = self.t0;
        p.started = self.started;
        p.clock = self.clock;
        Some(p)
    }

    /// Whether the scene is a journey (its whole frame moves every tick).
    pub fn is_journey(&self) -> bool {
        self.pkg
            .graph(self.theme)
            .is_some_and(|g| g.journey.is_some())
    }

    /// The size of what this player shows.
    pub fn size(&self) -> (u32, u32) {
        (self.view.w, self.view.h)
    }

    /// The whole current frame as palette indices (not just the view).
    pub fn image(&self) -> &IndexedImage {
        self.frame.image()
    }

    /// Palette indices of `w` pixels of row `y` from column `x`, in the
    /// view's coordinates.
    pub fn index_row(&self, x: u32, y: u32, w: u32) -> &[u8] {
        let img = self.frame.image();
        let a = (self.view.y + y) as usize * img.width as usize + (self.view.x + x) as usize;
        &img.pixels[a..a + w as usize]
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

    /// Moves to `t_ms`; returns the rectangles that changed, in the view's
    /// coordinates (changes outside the view are left out).
    pub fn advance(&mut self, t_ms: u64) -> Vec<PixelRect> {
        let rects = self
            .pkg
            .advance(&mut self.frame, self.theme, self.buddy, t_ms, self.clock)
            .unwrap_or_default();
        let v = self.view;
        if (v.x, v.y, v.w, v.h) == (0, 0, self.pkg.width, self.pkg.height) {
            return rects;
        }
        rects
            .iter()
            .filter_map(|r| {
                let x0 = r.x.max(v.x);
                let y0 = r.y.max(v.y);
                let x1 = (r.x + r.w).min(v.x + v.w);
                let y1 = (r.y + r.h).min(v.y + v.h);
                (x1 > x0 && y1 > y0).then(|| PixelRect {
                    x: x0 - v.x,
                    y: y0 - v.y,
                    w: x1 - x0,
                    h: y1 - y0,
                })
            })
            .collect()
    }

    /// Writes those rectangles (view coordinates) of the current frame into
    /// a 0RGB buffer of the view's size.
    pub fn write(&self, buf: &mut [u32], rects: &[PixelRect]) {
        let w = self.view.w as usize;
        debug_assert_eq!(buf.len(), w * self.view.h as usize);
        for r in rects {
            for y in r.y..r.y + r.h {
                let a = y as usize * w + r.x as usize;
                let row = self.index_row(r.x, y, r.w);
                for (out, &i) in buf[a..a + r.w as usize].iter_mut().zip(row) {
                    *out = self.colors[usize::from(i)];
                }
            }
        }
    }

    /// One rectangle (view coordinates) of the current frame as 0RGB
    /// pixels, row by row.
    pub fn rect_pixels(&self, r: &PixelRect, out: &mut Vec<u32>) {
        out.clear();
        for y in r.y..r.y + r.h {
            out.extend(
                self.index_row(r.x, y, r.w)
                    .iter()
                    .map(|&i| self.colors[usize::from(i)]),
            );
        }
    }

    /// Writes the whole view.
    pub fn write_all(&self, buf: &mut [u32]) {
        let whole = PixelRect {
            x: 0,
            y: 0,
            w: self.view.w,
            h: self.view.h,
        };
        self.write(buf, &[whole]);
    }
}

/// The highest frame rate for a journey: its land moves, so every frame is
/// the whole screen, and Very smooth already scrolls evenly.
pub const JOURNEY_MAX_FPS: u32 = 30;

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

    /// Span mode: a player showing part of a wide scene keeps a buffer of
    /// just that part exact, from its own changed rects, like a whole-frame
    /// player does.
    #[test]
    fn a_view_shows_its_part_of_the_frame_exactly() {
        let (w, h) = (640, 180);
        let r = compose(7, Aspect::from_size(w, h), &ContentContext::default());
        let pkg = Arc::new(ScenePackage::capture(&r, w, h).unwrap());
        let theme = pkg.themes()[0];
        // The right half, as a second monitor of the same height.
        let view = PixelRect {
            x: 320,
            y: 0,
            w: 320,
            h: 180,
        };
        let mut p = Player::new_view(pkg.clone(), theme, true, view).unwrap();
        assert_eq!(p.size(), (320, 180));
        let mut buf = vec![0u32; 320 * 180];
        let t0 = p.t0;
        p.advance(t0);
        p.write_all(&mut buf);
        let crop = |t: u64| -> Vec<u32> {
            let (full, palette) = pkg.frame(theme, true, t, Clock::Live).unwrap();
            let colors: Vec<u32> = palette
                .colors()
                .iter()
                .map(|c| (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b))
                .collect();
            (0..180)
                .flat_map(|y| (320..640).map(move |x| (x, y)))
                .map(|(x, y)| colors[usize::from(full.pixels[y * 640 + x])])
                .collect()
        };
        assert_eq!(buf, crop(t0));
        for k in 1..40u64 {
            let t = t0 + k * 83;
            let rects = p.advance(t);
            assert!(rects.iter().all(|r| r.x + r.w <= 320 && r.y + r.h <= 180));
            p.write(&mut buf, &rects);
            assert_eq!(buf, crop(t), "t {t}");
        }
        // A view outside the frame is refused.
        let outside = PixelRect {
            x: 400,
            y: 0,
            w: 320,
            h: 180,
        };
        assert!(Player::new_view(pkg, theme, true, outside).is_none());
    }
}
