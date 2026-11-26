//! Tile bitmask of dirty pixel regions for double-buffered CPU surfaces.
//!
//! Screen-wide weather and campfire embers produce hundreds of cell-sized
//! rectangles per frame, and a double-buffered surface accumulates two frames'
//! worth of them before swapping. Collapsing past 48 rectangles to one
//! bounding box turns sparse snow or spray into a full-screen copy every
//! frame (~24-33 MB on Retina/4K), while keeping a raw list rewrites pixels
//! where smooth-clock particles overlap from one frame to the next.
//!
//! [`DirtyGrid`] marks 32x16-pixel tiles in a fixed bitset (32 BGRA pixels =
//! 128 bytes, one Apple Silicon cache line) with no per-frame allocation, and
//! emits merged horizontal and vertical runs so every touched pixel is copied
//! at most once.

use clairlune_render::PixelRect;

/// 32 4-byte pixels = 128 bytes (one Apple Silicon cache line).
pub const TILE_W: u32 = 32;
pub const TILE_H: u32 = 16;

/// Fixed-allocation tile bitset covering a `width x height` surface.
#[derive(Clone, Debug)]
pub struct DirtyGrid {
    width: u32,
    height: u32,
    tile_cols: usize,
    tile_rows: usize,
    words_per_row: usize,
    bits: Vec<u64>,
    any: bool,
}

impl DirtyGrid {
    /// A grid with every tile initially marked dirty (for a fresh buffer's
    /// first present).
    pub fn new_full(width: u32, height: u32) -> Self {
        let tile_cols = width.div_ceil(TILE_W) as usize;
        let tile_rows = height.div_ceil(TILE_H) as usize;
        let words_per_row = tile_cols.div_ceil(64);
        let mut grid = Self {
            width,
            height,
            tile_cols,
            tile_rows,
            words_per_row,
            bits: vec![0u64; words_per_row * tile_rows],
            any: false,
        };
        grid.mark_all();
        grid
    }

    pub fn is_empty(&self) -> bool {
        !self.any
    }

    pub fn clear(&mut self) {
        if self.any {
            self.bits.fill(0);
            self.any = false;
        }
    }

    /// Marks the entire surface dirty.
    pub fn mark_all(&mut self) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let tail_bits = self.tile_cols % 64;
        let tail_mask = if tail_bits == 0 {
            u64::MAX
        } else {
            (1u64 << tail_bits) - 1
        };
        for ty in 0..self.tile_rows {
            let row = &mut self.bits[ty * self.words_per_row..(ty + 1) * self.words_per_row];
            if let Some((last, rest)) = row.split_last_mut() {
                rest.fill(u64::MAX);
                *last = tail_mask;
            }
        }
        self.any = true;
    }

    /// Marks every tile intersecting `r` (clipped to the surface).
    pub fn mark(&mut self, r: PixelRect) {
        let x0 = r.x.min(self.width);
        let y0 = r.y.min(self.height);
        let x1 = r.x.saturating_add(r.w).min(self.width);
        let y1 = r.y.saturating_add(r.h).min(self.height);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let tx0 = (x0 / TILE_W) as usize;
        let tx1 = x1.div_ceil(TILE_W) as usize;
        let ty0 = (y0 / TILE_H) as usize;
        let ty1 = y1.div_ceil(TILE_H) as usize;
        let (w0, b0) = (tx0 / 64, tx0 % 64);
        let (w1, b1) = ((tx1 - 1) / 64, (tx1 - 1) % 64);
        let first_mask = u64::MAX << b0;
        let last_mask = if b1 == 63 {
            u64::MAX
        } else {
            (1u64 << (b1 + 1)) - 1
        };
        let wpr = self.words_per_row;
        for ty in ty0..ty1 {
            let row = &mut self.bits[ty * wpr..(ty + 1) * wpr];
            if w0 == w1 {
                row[w0] |= first_mask & last_mask;
            } else {
                row[w0] |= first_mask;
                for word in &mut row[w0 + 1..w1] {
                    *word = u64::MAX;
                }
                row[w1] |= last_mask;
            }
        }
        self.any = true;
    }

    pub fn mark_slice(&mut self, rects: &[PixelRect]) {
        for &r in rects {
            self.mark(r);
        }
    }

    /// Calls `f` with non-overlapping rectangles covering every dirty tile,
    /// merging horizontally adjacent tiles and consecutive identical tile rows.
    pub fn for_each_rect(&self, mut f: impl FnMut(PixelRect)) {
        if !self.any {
            return;
        }
        let wpr = self.words_per_row;
        let mut ty = 0;
        while ty < self.tile_rows {
            let row = &self.bits[ty * wpr..(ty + 1) * wpr];
            if row.iter().all(|&w| w == 0) {
                ty += 1;
                continue;
            }
            let mut ty_end = ty + 1;
            while ty_end < self.tile_rows && self.bits[ty_end * wpr..(ty_end + 1) * wpr] == *row {
                ty_end += 1;
            }
            let y0 = (ty as u32 * TILE_H).min(self.height);
            let y1 = (ty_end as u32 * TILE_H).min(self.height);
            let h = y1 - y0;
            let mut tx = 0;
            while tx < self.tile_cols {
                let (wi, bi) = (tx / 64, tx % 64);
                let shifted = row[wi] >> bi;
                if shifted == 0 {
                    tx = (wi + 1) * 64;
                    continue;
                }
                tx += shifted.trailing_zeros() as usize;
                if tx >= self.tile_cols {
                    break;
                }
                let start = tx;
                loop {
                    let (w_idx, b_idx) = (tx / 64, tx % 64);
                    if w_idx >= wpr {
                        break;
                    }
                    let ones = (row[w_idx] >> b_idx).trailing_ones() as usize;
                    tx += ones;
                    if b_idx + ones < 64 || tx >= self.tile_cols {
                        break;
                    }
                }
                let end = tx.min(self.tile_cols);
                let x0 = (start as u32 * TILE_W).min(self.width);
                let x1 = (end as u32 * TILE_W).min(self.width);
                if x1 > x0 && h > 0 {
                    f(PixelRect {
                        x: x0,
                        y: y0,
                        w: x1 - x0,
                        h,
                    });
                }
            }
            ty = ty_end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Player;
    use clairlune_render::{ScenePackage, clock::Clock};
    use clairlune_scene::{Aspect, ContentContext, compose};
    use std::sync::Arc;

    #[test]
    fn scattered_weather_stays_sparse_instead_of_collapsing_to_the_whole_screen() {
        let (w, h) = (3024, 1964);
        let mut grid = DirtyGrid::new_full(w, h);
        // Initial state is one full-screen rectangle.
        let mut init = Vec::new();
        grid.for_each_rect(|r| init.push(r));
        assert_eq!(init, [PixelRect { x: 0, y: 0, w, h }]);
        grid.clear();
        assert!(grid.is_empty());

        // 240 scattered snow/sandstorm cells across the whole screen (far past
        // the old MAX_MISSING = 48 threshold, including every corner).
        for i in 0..240u32 {
            let x = (i * 7919) % (w - 10);
            let y = (i * 6151) % (h - 18);
            grid.mark(PixelRect { x, y, w: 10, h: 18 });
        }
        // Corners too, so a bounding box would cover all 5.94M pixels.
        grid.mark(PixelRect {
            x: 0,
            y: 0,
            w: 8,
            h: 8,
        });
        grid.mark(PixelRect {
            x: w - 8,
            y: h - 8,
            w: 8,
            h: 8,
        });
        let mut pixels = 0u64;
        grid.for_each_rect(|r| {
            assert!(r.x + r.w <= w && r.y + r.h <= h);
            pixels += u64::from(r.w * r.h);
        });
        let whole = u64::from(w) * u64::from(h);
        assert!(
            pixels < whole / 15,
            "expected sparse tiles (<7% of screen), got {pixels} of {whole}"
        );
    }

    #[test]
    fn double_buffered_dirty_grids_keep_both_surfaces_exact() {
        let (w, h) = (320, 180);
        let ctx = ContentContext {
            enabled_weather: vec!["sandstorm".into()],
            ..ContentContext::default()
        };
        let r = compose(19, Aspect::from_size(w, h), &ctx);
        let pkg = Arc::new(ScenePackage::capture(&r, w, h).unwrap());
        let theme = pkg.themes()[0];
        let mut player = Player::new(pkg.clone(), theme, true).unwrap();
        player.set_smooth(true);
        let mut bufs = [vec![0u32; (w * h) as usize], vec![0u32; (w * h) as usize]];
        let mut grids = [DirtyGrid::new_full(w, h), DirtyGrid::new_full(w, h)];
        let t0 = pkg.hero_ms(theme).unwrap();

        for step in 0..30u64 {
            let t = t0 + step * 33;
            let rects = player.advance(t);
            for g in &mut grids {
                g.mark_slice(&rects);
            }
            let cur = (step % 2) as usize;
            let colors = *player.colors();
            grids[cur].for_each_rect(|rect| {
                for y in rect.y..rect.y + rect.h {
                    let off = (y * w + rect.x) as usize;
                    for (dst, &idx) in bufs[cur][off..off + rect.w as usize]
                        .iter_mut()
                        .zip(player.index_row(rect.x, y, rect.w))
                    {
                        *dst = colors[usize::from(idx)];
                    }
                }
            });
            grids[cur].clear();

            let (expected, palette) = pkg.frame(theme, true, t, Clock::Smooth).unwrap();
            let pal: Vec<u32> = palette
                .colors()
                .iter()
                .map(|c| (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b))
                .collect();
            let want: Vec<u32> = expected
                .pixels
                .iter()
                .map(|&i| pal[usize::from(i)])
                .collect();
            assert_eq!(bufs[cur], want, "frame {step} at {t}");
        }
    }
}
