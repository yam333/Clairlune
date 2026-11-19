//! The app's own mark: a pixel-art crescent moon ("clair de lune"). Clawd is
//! Anthropic's character and stays in the wallpapers; the app icon must not
//! borrow it.

/// A crescent moon as RGBA, `size` x `size` pixels, hard-edged like the
/// wallpapers' pixel art: an ivory disc cut by an offset disc, with a slightly
/// darker rim.
pub fn moon_rgba(size: u32) -> Vec<u8> {
    let s = f64::from(size);
    let (cx, cy, r) = (s * 0.5, s * 0.5, s * 0.42);
    let (sx, sy, sr) = (s * 0.68, s * 0.36, s * 0.36);
    const FACE: [u8; 4] = [0xF2, 0xE6, 0xC9, 0xFF];
    const RIM: [u8; 4] = [0xC9, 0xB8, 0x94, 0xFF];
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let ds = ((fx - sx).powi(2) + (fy - sy).powi(2)).sqrt();
            if d > r || ds < sr {
                continue;
            }
            let rim = d > r - s / 16.0 || ds < sr + s / 16.0;
            let i = ((y * size + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(if rim { &RIM } else { &FACE });
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crescent_with_transparent_corners() {
        let px = moon_rgba(32);
        assert_eq!(px.len(), 32 * 32 * 4);
        let alpha = |x: u32, y: u32| px[((y * 32 + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(16, 16), 0, "the bite out of the moon");
        assert_eq!(alpha(7, 20), 0xFF, "the lit limb");
        let opaque = px.chunks(4).filter(|p| p[3] == 0xFF).count();
        assert!((150..600).contains(&opaque), "{opaque}");
    }
}
