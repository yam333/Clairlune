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

    fn png_of(size: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&moon_rgba(size))
            .unwrap();
        out
    }

    /// An .ico with a PNG image per size, each drawn at that size (resizing
    /// one big image would blur the pixel art).
    fn ico() -> Vec<u8> {
        let sizes = [16u32, 24, 32, 48, 64, 128, 256];
        let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png_of(s)).collect();
        let mut out = Vec::new();
        out.extend_from_slice(&[0, 0, 1, 0]);
        out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * sizes.len() as u32;
        for (s, img) in sizes.iter().zip(&images) {
            let dim = if *s >= 256 { 0 } else { *s as u8 };
            out.extend_from_slice(&[dim, dim, 0, 0]);
            out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
            out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
            out.extend_from_slice(&(img.len() as u32).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            offset += img.len() as u32;
        }
        for img in &images {
            out.extend_from_slice(img);
        }
        out
    }

    /// The Windows exe icon (`packaging/icons/clairlune.ico`), same rule.
    #[test]
    fn the_exe_icon_is_the_moon() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/icons/clairlune.ico");
        if std::env::var_os("CLAIRLUNE_WRITE_ICONS").is_some() {
            std::fs::write(&path, ico()).unwrap();
            return;
        }
        assert!(
            std::fs::read(&path).unwrap() == ico(),
            "{} differs from the moon",
            path.display()
        );
    }

    /// The installer icons (`packaging/icons/<size>.png`) are this moon:
    /// `CLAIRLUNE_WRITE_ICONS=1 cargo test -p clairlune icon` writes them, and
    /// otherwise this checks the committed ones still match.
    #[test]
    fn installer_icons_are_the_moon() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/icons");
        for size in [32u32, 64, 128, 256, 512, 1024] {
            let rgba = moon_rgba(size);
            let path = dir.join(format!("{size}.png"));
            if std::env::var_os("CLAIRLUNE_WRITE_ICONS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
                let mut enc = png::Encoder::new(file, size, size);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().unwrap().write_image_data(&rgba).unwrap();
                continue;
            }
            let dec = png::Decoder::new(std::io::BufReader::new(
                std::fs::File::open(&path).unwrap_or_else(|_| panic!("missing {}", path.display())),
            ));
            let mut reader = dec.read_info().unwrap();
            let mut buf = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut buf).unwrap();
            buf.truncate(info.buffer_size());
            assert!(buf == rgba, "{} differs from the moon", path.display());
        }
    }

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
