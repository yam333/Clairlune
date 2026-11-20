//! Seamless GIF export (PLAN.md section 6.4).
//!
//! - One global palette: the scene palette (at most 255 colours, by
//!   construction) plus index 255 as the transparent colour. No per-frame
//!   quantisation, no dithering.
//! - Frame 0 is the full image. Every later frame is exactly one sub-image: the
//!   bounding box of pixels that changed, with unchanged pixels inside it set to
//!   the transparent index (helps LZW), `DisposalMethod::Keep`. A frame identical
//!   to the previous one is merged into the previous frame's delay.
//! - Delays are whole centiseconds, dithered so they sum exactly to the loop.
//! - The loop length is a multiple of the particle period, and the character
//!   follows its loop script (`Clock::Loop`), so the last frame flows into the
//!   first with no seam.

use crate::clock::Clock;
use crate::frame::render_frame_clocked;
use crate::palette::TRANSPARENT_INDEX;
use crate::{GlyphAtlas, IndexedImage, IndexedPalette, bake_static};
use clairlune_scene::particles::LOOP_MS;
use clairlune_scene::{BuildError, BuildOptions, SceneRecipe, Theme, build};
use std::fmt;
use std::io::Write;
use std::time::Instant;

/// Smoothness (PLAN.md section 5.3), capped by what GIF delays can express.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Smoothness {
    BatterySaver,
    Balanced,
    Smooth,
    VerySmooth,
    UltraSmooth,
}

impl Smoothness {
    /// Target delay per frame in centiseconds: about 6, 12, 25, 33 and 50 fps.
    pub fn gif_delay_cs(self) -> u32 {
        match self {
            Smoothness::BatterySaver => 16,
            Smoothness::Balanced => 8,
            Smoothness::Smooth => 4,
            Smoothness::VerySmooth => 3,
            Smoothness::UltraSmooth => 2,
        }
    }
}

/// Valid loop lengths: whole multiples of the particle period.
pub fn valid_loop_ms(loop_ms: u64) -> bool {
    loop_ms > 0 && loop_ms.is_multiple_of(LOOP_MS)
}

/// Per-frame delays in centiseconds that sum exactly to the loop, as even as
/// possible around the target (e.g. a mix of 16 and 17 cs).
pub fn frame_delays(loop_ms: u64, target_cs: u32) -> Vec<u16> {
    let total = loop_ms / 10;
    let n = ((total as f64 / f64::from(target_cs.max(1))).round() as u64).max(1);
    (0..n)
        .map(|i| ((i + 1) * total / n - i * total / n) as u16)
        .collect()
}

#[derive(Debug)]
pub enum GifError {
    Build(BuildError),
    Encode(gif::EncodingError),
    LoopLength(u64),
    TooLarge { width: u32, height: u32 },
}

impl fmt::Display for GifError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GifError::Build(e) => write!(f, "{e}"),
            GifError::Encode(e) => write!(f, "GIF encoding failed: {e}"),
            GifError::LoopLength(ms) => write!(
                f,
                "loop length {ms} ms isn't a multiple of the {LOOP_MS} ms particle period, so it couldn't loop seamlessly"
            ),
            GifError::TooLarge { width, height } => {
                write!(
                    f,
                    "{width}x{height} is larger than GIF allows (65535 px per side)"
                )
            }
        }
    }
}

impl std::error::Error for GifError {}

impl From<gif::EncodingError> for GifError {
    fn from(e: gif::EncodingError) -> Self {
        GifError::Encode(e)
    }
}

/// What an export produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GifStats {
    pub width: u32,
    pub height: u32,
    pub loop_ms: u64,
    pub frames_rendered: usize,
    pub frames_written: usize,
    pub elapsed_ms: u64,
}

/// Bounding box `(x0, y0, x1, y1)` (exclusive end) of pixels that differ.
fn changed_box(a: &IndexedImage, b: &IndexedImage) -> Option<(u32, u32, u32, u32)> {
    let w = a.width as usize;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for (y, (ra, rb)) in a
        .pixels
        .chunks_exact(w)
        .zip(b.pixels.chunks_exact(w))
        .enumerate()
    {
        if ra == rb {
            continue;
        }
        let first = ra.iter().zip(rb).position(|(p, q)| p != q).unwrap();
        let last = w
            - 1
            - ra.iter()
                .rev()
                .zip(rb.iter().rev())
                .position(|(p, q)| p != q)
                .unwrap();
        x0 = x0.min(first);
        x1 = x1.max(last + 1);
        y0 = y0.min(y);
        y1 = y + 1;
    }
    (x0 != usize::MAX).then_some((x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Renders a seamless looping GIF of a recipe.
pub fn encode_gif<W: Write>(
    recipe: &SceneRecipe,
    opts: &BuildOptions,
    loop_ms: u64,
    smoothness: Smoothness,
    out: W,
) -> Result<GifStats, GifError> {
    if !valid_loop_ms(loop_ms) {
        return Err(GifError::LoopLength(loop_ms));
    }
    if opts.width > u32::from(u16::MAX) || opts.height > u32::from(u16::MAX) {
        return Err(GifError::TooLarge {
            width: opts.width,
            height: opts.height,
        });
    }
    let started = Instant::now();
    let graph = build(recipe, opts).map_err(GifError::Build)?;
    let atlas = GlyphAtlas::for_grid(&graph.grid);
    let palette = IndexedPalette::new(&graph.family);
    let background = bake_static(&graph, &atlas);
    let clock = Clock::Loop(loop_ms);
    let (rendered, written) = encode_frames(
        opts.width,
        opts.height,
        &palette,
        loop_ms,
        smoothness,
        out,
        |t| render_frame_clocked(&background, &graph, &atlas, &palette, t, clock),
    )?;
    let stats = GifStats {
        width: opts.width,
        height: opts.height,
        loop_ms,
        frames_rendered: rendered,
        frames_written: written,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    tracing::info!(
        target: "clairlune_render",
        seed = recipe.seed,
        recipe_hash = %recipe.hash_hex(),
        width = opts.width,
        height = opts.height,
        loop_ms,
        frames_rendered = stats.frames_rendered,
        frames_written = stats.frames_written,
        elapsed_ms = stats.elapsed_ms,
        "gif encoded"
    );
    Ok(stats)
}

/// Writes a looping GIF from a frame source: one global palette, changed
/// boxes only, identical frames merged. Returns frames rendered and written.
fn encode_frames<W: Write>(
    width: u32,
    height: u32,
    palette: &IndexedPalette,
    loop_ms: u64,
    smoothness: Smoothness,
    out: W,
    mut frame_at: impl FnMut(u64) -> IndexedImage,
) -> Result<(usize, usize), GifError> {
    let mut global = palette.rgb_bytes();
    global.resize(256 * 3, 0); // index 255 stays free as the transparent colour
    let (w16, h16) = (width as u16, height as u16);
    let mut enc = gif::Encoder::new(out, w16, h16, &global)?;
    enc.set_repeat(gif::Repeat::Infinite)?;
    let delays = frame_delays(loop_ms, smoothness.gif_delay_cs());
    let mut prev: Option<IndexedImage> = None;
    let mut pending: Option<gif::Frame<'static>> = None;
    let mut written = 0usize;
    let mut t_cs = 0u64;
    for &d in &delays {
        let img = frame_at(t_cs * 10);
        t_cs += u64::from(d);
        let frame = match &prev {
            None => gif::Frame {
                width: w16,
                height: h16,
                delay: d,
                dispose: gif::DisposalMethod::Keep,
                buffer: img.pixels.clone().into(),
                ..gif::Frame::default()
            },
            Some(p) => match changed_box(p, &img) {
                None => {
                    // Identical frame: extend the previous frame instead.
                    if let Some(f) = pending.as_mut() {
                        f.delay += d;
                    }
                    continue;
                }
                Some((x0, y0, x1, y1)) => {
                    let (bw, bh) = (x1 - x0, y1 - y0);
                    let mut buf = Vec::with_capacity((bw * bh) as usize);
                    for y in y0..y1 {
                        let row = (y * img.width) as usize;
                        for x in x0..x1 {
                            let i = row + x as usize;
                            buf.push(if img.pixels[i] == p.pixels[i] {
                                TRANSPARENT_INDEX
                            } else {
                                img.pixels[i]
                            });
                        }
                    }
                    gif::Frame {
                        left: x0 as u16,
                        top: y0 as u16,
                        width: bw as u16,
                        height: bh as u16,
                        delay: d,
                        dispose: gif::DisposalMethod::Keep,
                        transparent: Some(TRANSPARENT_INDEX),
                        buffer: buf.into(),
                        ..gif::Frame::default()
                    }
                }
            },
        };
        if let Some(f) = pending.take() {
            enc.write_frame(&f)?;
            written += 1;
        }
        pending = Some(frame);
        prev = Some(img);
    }
    if let Some(f) = pending.take() {
        enc.write_frame(&f)?;
        written += 1;
    }
    drop(enc);
    Ok((delays.len(), written))
}

/// Exports a saved scene as a looping GIF, exactly as saved (no generator).
pub fn encode_package_gif<W: Write>(
    pkg: &crate::ScenePackage,
    theme: Theme,
    buddy: bool,
    loop_ms: u64,
    smoothness: Smoothness,
    out: W,
) -> Result<GifStats, GifError> {
    if !valid_loop_ms(loop_ms) {
        return Err(GifError::LoopLength(loop_ms));
    }
    if pkg.width > u32::from(u16::MAX) || pkg.height > u32::from(u16::MAX) {
        return Err(GifError::TooLarge {
            width: pkg.width,
            height: pkg.height,
        });
    }
    let palette = pkg
        .palette(theme)
        .ok_or(GifError::Build(BuildError::ThemeLocked(theme)))?
        .clone();
    let started = Instant::now();
    let clock = Clock::Loop(loop_ms);
    let (rendered, written) = encode_frames(
        pkg.width,
        pkg.height,
        &palette,
        loop_ms,
        smoothness,
        out,
        |t| {
            pkg.frame(theme, buddy, t, clock)
                .map(|(img, _)| img)
                .expect("theme checked above")
        },
    )?;
    Ok(GifStats {
        width: pkg.width,
        height: pkg.height,
        loop_ms,
        frames_rendered: rendered,
        frames_written: written,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_sum_exactly_to_the_loop() {
        for loop_ms in [6000u64, 12000] {
            for s in [
                Smoothness::BatterySaver,
                Smoothness::Balanced,
                Smoothness::Smooth,
                Smoothness::VerySmooth,
                Smoothness::UltraSmooth,
            ] {
                let d = frame_delays(loop_ms, s.gif_delay_cs());
                assert_eq!(d.iter().map(|&x| u64::from(x)).sum::<u64>(), loop_ms / 10);
                let (min, max) = (*d.iter().min().unwrap(), *d.iter().max().unwrap());
                assert!(max - min <= 1, "{s:?}: delays {min}..{max}");
                assert!(u32::from(min) >= 2, "browsers clamp delays under 2 cs");
            }
        }
        // 6 s at Battery saver: 37.5 frames of 16 cs, so a 16/17 mix.
        let d = frame_delays(6000, 16);
        assert!(d.contains(&16) && d.contains(&15) || d.contains(&16) && d.contains(&17));
    }

    #[test]
    fn rejects_loops_that_cannot_be_seamless() {
        assert!(valid_loop_ms(6000) && valid_loop_ms(12000));
        assert!(!valid_loop_ms(3000) && !valid_loop_ms(10000) && !valid_loop_ms(0));
    }
}
