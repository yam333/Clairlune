//! Moonglass (docs/dev/design.md): the Settings window as glass laid over the
//! scene the app just drew. The sidebar is reeded glass: flutes that each
//! show the scene behind them a little wider, so the landscape breaks into
//! soft ribbons. The content pane is smoked glass over a blurred copy. With
//! no wallpaper yet, the glass is its own colour and keeps its reeds.

use super::style::{alpha, c};
use crate::tokens::{Colors, flute};
use egui::{
    Color32, ColorImage, Mesh, Painter, Pos2, Rect, TextureHandle, TextureOptions, pos2, vec2,
};

/// The scene behind the glass, sharp for the flutes and soft (blurred) for
/// the pane.
pub struct Backdrop {
    sharp: TextureHandle,
    soft: TextureHandle,
    /// Width over height.
    aspect: f32,
}

impl Backdrop {
    /// The wallpaper itself, sharp.
    pub fn image(&self) -> &TextureHandle {
        &self.sharp
    }

    /// From an RGBA image (the wallpaper on screen): a copy at most 1600 px
    /// wide for the flutes, and a small blurred one for the pane.
    pub fn new(ctx: &egui::Context, size: [usize; 2], rgba: &[u8]) -> Option<Backdrop> {
        if size[0] == 0 || size[1] == 0 || rgba.len() != size[0] * size[1] * 4 {
            return None;
        }
        let (sharp_size, sharp) = shrink(size, rgba, 1600);
        let (soft_size, soft) = shrink(size, rgba, 160);
        let soft = blur(soft_size, &soft, 3);
        let opts = TextureOptions::LINEAR;
        Some(Backdrop {
            sharp: ctx.load_texture(
                "glass-sharp",
                ColorImage::from_rgba_unmultiplied(sharp_size, &sharp),
                opts,
            ),
            soft: ctx.load_texture(
                "glass-soft",
                ColorImage::from_rgba_unmultiplied(soft_size, &soft),
                opts,
            ),
            aspect: size[0] as f32 / size[1] as f32,
        })
    }
}

/// A tile of fine grain, laid over the glass (made, not shipped).
pub fn grain(ctx: &egui::Context) -> TextureHandle {
    const N: usize = 128;
    let mut px = Vec::with_capacity(N * N * 4);
    let mut x: u32 = 0x9E37_79B9;
    for _ in 0..N * N {
        // xorshift: the same grain every time.
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let v = (x >> 24) as u8;
        let level = if v & 1 == 0 { 255 } else { 0 };
        px.extend_from_slice(&[level, level, level, v]);
    }
    ctx.load_texture(
        "glass-grain",
        ColorImage::from_rgba_unmultiplied([N, N], &px),
        TextureOptions::LINEAR_REPEAT,
    )
}

/// Paints the glass: the reeded sidebar and the smoked pane, over the
/// scene where there is one.
pub fn paint(
    p: &Painter,
    t: &Colors,
    screen: Rect,
    sidebar: Rect,
    pane: Rect,
    backdrop: Option<&Backdrop>,
    grain: Option<&TextureHandle>,
) {
    let g = &t.glass;
    match backdrop {
        Some(b) => {
            let img = cover(screen, b.aspect);
            p.image(b.soft.id(), pane, uv_rect(img, pane), Color32::WHITE);
            let (v0, v1) = (uv(img, sidebar.min).y, uv(img, sidebar.max).y);
            for (x0, x1) in flutes(sidebar) {
                // The scene under this flute's middle, `MAGNIFY` times
                // wider: what a rounded reed does to what's behind it.
                let mid = uv(img, pos2((x0 + x1) / 2.0, sidebar.min.y)).x;
                let half = (x1 - x0) / 2.0 / img.width() / flute::MAGNIFY;
                p.image(
                    b.sharp.id(),
                    Rect::from_min_max(pos2(x0, sidebar.min.y), pos2(x1, sidebar.max.y)),
                    Rect::from_min_max(pos2(mid - half, v0), pos2(mid + half, v1)),
                    Color32::WHITE,
                );
            }
        }
        None => {
            p.rect_filled(sidebar, 0.0, c(t.bg_sidebar));
            p.rect_filled(pane, 0.0, c(t.bg));
        }
    }
    // Each flute's lit left edge and shaded right edge.
    let mut m = Mesh::default();
    let light = alpha(WHITE, g.flute_light);
    let light_soft = alpha(WHITE, g.flute_light * 0.25);
    let shade = alpha(g.shade, g.flute_shade);
    let clear = alpha(g.shade, 0.0);
    for (x0, x1) in flutes(sidebar) {
        let w = x1 - x0;
        let (y0, y1) = (sidebar.min.y, sidebar.max.y);
        hquad(&mut m, x0, x0 + w * 0.28, y0, y1, light, light_soft);
        hquad(
            &mut m,
            x0 + w * 0.28,
            x0 + w * 0.52,
            y0,
            y1,
            light_soft,
            clear,
        );
        hquad(&mut m, x0 + w * 0.52, x1, y0, y1, clear, shade);
    }
    // The sidebar's tint, deeper toward the bottom; the pane's smoke.
    let (top, bottom) = if backdrop.is_some() {
        (g.tint_top, g.tint_bottom)
    } else {
        (g.tint_top * 0.4, g.tint_bottom * 0.4)
    };
    vquad(&mut m, sidebar, alpha(g.tint, top), alpha(g.tint, bottom));
    if backdrop.is_some() {
        m.add_colored_rect(pane, alpha(g.pane, g.pane_alpha));
    }
    p.add(m);
    if let Some(grain) = grain {
        for (r, a) in [(sidebar, g.grain), (pane, g.grain * 0.6)] {
            let uv = Rect::from_min_size(Pos2::ZERO, r.size() / 128.0);
            p.image(
                grain.id(),
                r,
                uv,
                Color32::from_white_alpha((a * 255.0) as u8),
            );
        }
    }
    // Where the glass meets the pane: a fine bright edge.
    p.vline(
        sidebar.max.x - 0.5,
        sidebar.y_range(),
        egui::Stroke::new(1.0, alpha(g.card, g.card_alpha * 2.0)),
    );
}

/// White, as an `Rgb` for [`alpha`].
const WHITE: crate::tokens::Rgb = crate::tokens::Rgb(255, 255, 255);

/// The flutes across a rect, left to right (the last may be narrower).
fn flutes(r: Rect) -> impl Iterator<Item = (f32, f32)> {
    let n = (r.width() / flute::WIDTH).ceil().max(0.0) as usize;
    (0..n).map(move |i| {
        let x0 = r.min.x + i as f32 * flute::WIDTH;
        (x0, (x0 + flute::WIDTH).min(r.max.x))
    })
}

/// Where an image of this aspect lies when it covers `screen`: centred,
/// cropped, never stretched.
fn cover(screen: Rect, aspect: f32) -> Rect {
    let (w, h) = (screen.width(), screen.height().max(1.0));
    let size = if w / h > aspect {
        vec2(w, w / aspect)
    } else {
        vec2(h * aspect, h)
    };
    Rect::from_center_size(screen.center(), size)
}

fn uv(img: Rect, p: Pos2) -> Pos2 {
    pos2(
        (p.x - img.min.x) / img.width(),
        (p.y - img.min.y) / img.height(),
    )
}

fn uv_rect(img: Rect, r: Rect) -> Rect {
    Rect::from_min_max(uv(img, r.min), uv(img, r.max))
}

/// A rectangle shaded from `left` to `right`.
fn hquad(m: &mut Mesh, x0: f32, x1: f32, y0: f32, y1: f32, left: Color32, right: Color32) {
    let i = m.vertices.len() as u32;
    m.colored_vertex(pos2(x0, y0), left);
    m.colored_vertex(pos2(x1, y0), right);
    m.colored_vertex(pos2(x1, y1), right);
    m.colored_vertex(pos2(x0, y1), left);
    m.add_triangle(i, i + 1, i + 2);
    m.add_triangle(i, i + 2, i + 3);
}

/// A rectangle shaded from `top` to `bottom`.
fn vquad(m: &mut Mesh, r: Rect, top: Color32, bottom: Color32) {
    let i = m.vertices.len() as u32;
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(i, i + 1, i + 2);
    m.add_triangle(i, i + 2, i + 3);
}

/// An RGBA image at most `max` px wide, each pixel the mean of the ones it
/// covers.
pub(crate) fn shrink(size: [usize; 2], rgba: &[u8], max: usize) -> ([usize; 2], Vec<u8>) {
    let [w, h] = size;
    if w <= max {
        return (size, rgba.to_vec());
    }
    let ow = max;
    let oh = ((h * max) / w).max(1);
    let mut out = vec![0u8; ow * oh * 4];
    for oy in 0..oh {
        let (sy0, sy1) = (oy * h / oh, ((oy + 1) * h / oh).max(oy * h / oh + 1));
        for ox in 0..ow {
            let (sx0, sx1) = (ox * w / ow, ((ox + 1) * w / ow).max(ox * w / ow + 1));
            let mut acc = [0u32; 4];
            for sy in sy0..sy1 {
                let row = &rgba[(sy * w + sx0) * 4..(sy * w + sx1) * 4];
                for px in row.as_chunks::<4>().0 {
                    for k in 0..4 {
                        acc[k] += u32::from(px[k]);
                    }
                }
            }
            let n = ((sy1 - sy0) * (sx1 - sx0)) as u32;
            let o = (oy * ow + ox) * 4;
            for k in 0..4 {
                out[o + k] = (acc[k] / n) as u8;
            }
        }
    }
    ([ow, oh], out)
}

/// A box blur of radius `r`, twice over (near enough a Gaussian), edges
/// held.
fn blur(size: [usize; 2], rgba: &[u8], r: usize) -> Vec<u8> {
    let [w, h] = size;
    let mut a = rgba.to_vec();
    let mut b = vec![0u8; a.len()];
    for _ in 0..2 {
        box_pass(&a, &mut b, w, h, r, true);
        box_pass(&b, &mut a, w, h, r, false);
    }
    a
}

fn box_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let at = |line: usize, i: usize| -> usize {
        if horizontal {
            (line * w + i) * 4
        } else {
            (i * w + line) * 4
        }
    };
    for line in 0..lines {
        for i in 0..len {
            let lo = i.saturating_sub(r);
            let hi = (i + r).min(len - 1);
            let mut acc = [0u32; 4];
            for j in lo..=hi {
                let o = at(line, j);
                for k in 0..4 {
                    acc[k] += u32::from(src[o + k]);
                }
            }
            let n = (hi - lo + 1) as u32;
            let o = at(line, i);
            for k in 0..4 {
                dst[o + k] = (acc[k] / n) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_averages_and_keeps_the_shape() {
        // 4x2 of alternating black and white: half as wide is all grey.
        let mut px = Vec::new();
        for i in 0..8 {
            let v = if i % 2 == 0 { 0 } else { 254 };
            px.extend_from_slice(&[v, v, v, 255]);
        }
        let (size, out) = shrink([4, 2], &px, 2);
        assert_eq!(size, [2, 1]);
        assert!(out.chunks(4).all(|p| p[0] == 127 && p[3] == 255), "{out:?}");
    }

    #[test]
    fn blurring_keeps_a_flat_colour_and_spreads_a_dot() {
        let flat = vec![90u8; 6 * 5 * 4];
        assert_eq!(blur([6, 5], &flat, 2), flat);
        let mut dot = vec![0u8; 9 * 9 * 4];
        dot[(4 * 9 + 4) * 4] = 255;
        let out = blur([9, 9], &dot, 1);
        assert!(out[(4 * 9 + 4) * 4] < 255 && out[(4 * 9 + 5) * 4] > 0);
    }

    #[test]
    fn the_cover_fills_the_screen_without_stretching() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(900.0, 640.0));
        for aspect in [16.0 / 9.0, 0.5, 900.0 / 640.0] {
            let img = cover(screen, aspect);
            assert!(img.width() >= 899.9 && img.height() >= 639.9);
            assert!((img.width() / img.height() - aspect).abs() < 1e-3);
            assert!((img.center() - screen.center()).length() < 1e-3);
        }
    }

    #[test]
    fn flutes_tile_the_sidebar() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(240.0, 10.0));
        let f: Vec<_> = flutes(r).collect();
        assert_eq!(f.len(), 20);
        assert_eq!(f[0], (0.0, 12.0));
        assert_eq!(f.last().copied(), Some((228.0, 240.0)));
    }
}
