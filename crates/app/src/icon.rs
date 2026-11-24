//! The app's own mark: a square over a circle, the moon behind a screen.
//! Clawd is Anthropic's character and stays in the wallpapers; the app icon
//! must not borrow it.
//!
//! The square's lower-left corner is the circle's centre.

const MOON: [u8; 3] = [0xF3, 0xEB, 0xDD];
const SQUARE: [u8; 3] = [0xE5, 0x82, 0x58];
/// The tile behind the app icon, top and bottom.
const TILE: [[u8; 3]; 2] = [[0x2A, 0x2B, 0x35], [0x12, 0x13, 0x18]];

/// The mark in a box `side` wide: the square's edges on whole pixels.
struct Mark {
    /// The square's left and bottom edges, and the circle's centre.
    x: f64,
    y: f64,
    /// The square's side, its corners' radius, and the circle's radius.
    side: f64,
    corner: f64,
    r: f64,
}

impl Mark {
    /// `at` is the box's top left corner.
    fn new(side: f64, at: f64) -> Mark {
        let k = side / 32.0;
        let square = (16.0 * k).round();
        Mark {
            x: (at + 30.0 * k).round() - square,
            y: (at + 2.0 * k).round() + square,
            side: square,
            corner: 2.0 * k,
            r: 12.0 * k,
        }
    }

    fn circle(&self, x: f64, y: f64) -> bool {
        (x - self.x).powi(2) + (y - self.y).powi(2) <= self.r * self.r
    }

    /// The square, `gap` larger all round.
    fn square(&self, x: f64, y: f64, gap: f64) -> bool {
        rounded(
            x - (self.x - gap),
            y - (self.y - self.side - gap),
            self.side + 2.0 * gap,
            self.corner + gap,
        )
    }

    fn colour(&self, x: f64, y: f64) -> Option<[u8; 3]> {
        if self.square(x, y, 0.0) {
            Some(SQUARE)
        } else if self.circle(x, y) {
            Some(MOON)
        } else {
            None
        }
    }
}

/// Inside a square of `side` at the origin, its corners rounded.
fn rounded(x: f64, y: f64, side: f64, corner: f64) -> bool {
    if x < 0.0 || y < 0.0 || x > side || y > side {
        return false;
    }
    let dx = (corner - x).max(x - (side - corner)).max(0.0);
    let dy = (corner - y).max(y - (side - corner)).max(0.0);
    dx * dx + dy * dy <= corner * corner
}

/// An image with smooth edges, like a vector icon drawn at `size`: each pixel
/// is the mean of 16 samples.
fn sampled(size: u32, at: impl Fn(f64, f64) -> Option<[u8; 3]>) -> Vec<u8> {
    const N: u32 = 4;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for (k, out) in px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (k as u32 % size, k as u32 / size);
        let mut sum = [0u32; 3];
        let mut inside = 0u32;
        for j in 0..N {
            for i in 0..N {
                let fx = f64::from(x) + (f64::from(i) + 0.5) / f64::from(N);
                let fy = f64::from(y) + (f64::from(j) + 0.5) / f64::from(N);
                if let Some(rgb) = at(fx, fy) {
                    inside += 1;
                    for (s, v) in sum.iter_mut().zip(rgb) {
                        *s += u32::from(v);
                    }
                }
            }
        }
        if inside == 0 {
            continue;
        }
        for (o, s) in out.iter_mut().zip(sum) {
            *o = ((s + inside / 2) / inside) as u8;
        }
        out[3] = ((inside * 255 + N * N / 2) / (N * N)) as u8;
    }
    px
}

/// The mark on its dark tile, which starts `margin` pixels in.
fn tile_rgba(size: u32, margin: f64) -> Vec<u8> {
    let side = f64::from(size) - 2.0 * margin;
    // The mark's box is a little smaller than the tile.
    let inset = side * 0.14;
    let m = Mark::new(side - 2.0 * inset, margin + inset);
    sampled(size, |x, y| {
        if !rounded(x - margin, y - margin, side, side * 0.225) {
            return None;
        }
        m.colour(x, y).or_else(|| {
            let t = (y - margin) / side;
            let mut rgb = TILE[0];
            for (v, to) in rgb.iter_mut().zip(TILE[1]) {
                *v = (f64::from(*v) + (f64::from(to) - f64::from(*v)) * t).round() as u8;
            }
            Some(rgb)
        })
    })
}

/// The app icon (Windows: the exe, the title bar, the taskbar): the mark on
/// its tile, edge to edge.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    tile_rgba(size, 0.0)
}

/// The app icon with the margin macOS gives its icons (824 of 1024).
#[cfg(test)]
fn mac_icon_rgba(size: u32) -> Vec<u8> {
    tile_rgba(size, f64::from(size) * 100.0 / 1024.0)
}

/// The running-in-the-background mark (tray, menu bar): one colour, with a
/// clear line around the square where it covers the circle.
pub fn tray_rgba(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let m = Mark::new(f64::from(size), 0.0);
    let gap = (f64::from(size) / 16.0).round().max(1.0);
    sampled(size, |x, y| {
        let shown = m.square(x, y, 0.0) || (m.circle(x, y) && !m.square(x, y, gap));
        shown.then_some(rgb)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(px: &[u8], size: u32, x: u32, y: u32) -> u8 {
        px[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn the_tray_mark_is_one_colour_with_smooth_edges() {
        let px = tray_rgba(32, [255, 255, 255]);
        let pixels: Vec<&[u8]> = px.chunks(4).collect();
        // One colour wherever anything shows.
        assert!(
            pixels
                .iter()
                .filter(|p| p[3] > 0)
                .all(|p| p[..3] == [255, 255, 255])
        );
        assert!(pixels.iter().any(|p| (1..255).contains(&p[3])));
    }

    #[test]
    fn a_clear_line_parts_the_square_from_the_circle() {
        // The square is x 14..30, y 2..18; the circle is at its lower left
        // corner.
        for (size, k) in [(16u32, 1u32), (32, 2)] {
            let px = tray_rgba(size, [0, 0, 0]);
            let at = |x: u32, y: u32| alpha(&px, size, x, y);
            assert_eq!(at(0, 0), 0);
            assert_eq!(at(11 * k, 5 * k), 255, "the square");
            assert_eq!(at(7 * k, 5 * k), 255, "its left edge, on a whole pixel");
            assert_eq!(at(11 * k, 9 * k - 1), 255, "its bottom edge");
            assert_eq!(at(7 * k - 1, 5 * k), 0, "the line beside it");
            assert_eq!(at(9 * k, 9 * k), 0, "the line under it");
            assert_eq!(at(4 * k, 9 * k), 255, "the circle");
            assert_eq!(at(7 * k, 12 * k), 255, "the circle");
        }
    }

    #[test]
    fn the_app_icon_is_an_orange_square_over_a_pale_circle_on_its_tile() {
        let px = app_icon_rgba(64);
        assert_eq!(px.len(), 64 * 64 * 4);
        let at = |x: u32, y: u32| &px[((y * 64 + x) * 4) as usize..][..4];
        assert_eq!(at(0, 0)[3], 0);
        assert_eq!(at(4, 32)[3], 255);
        assert_eq!(at(44, 20)[..3], SQUARE);
        assert_eq!(at(31, 32)[..3], SQUARE, "over the circle");
        assert_eq!(at(22, 40)[..3], MOON);
        // The Mac's has a margin around the tile.
        let mac = mac_icon_rgba(64);
        assert_eq!(alpha(&mac, 64, 4, 32), 0);
        assert_eq!(alpha(&mac, 64, 8, 32), 255);
    }

    fn png_of(size: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(rgba).unwrap();
        out
    }

    /// An .ico with a PNG image per size, each drawn at that size (resizing
    /// one big image would blur its edges).
    fn ico() -> Vec<u8> {
        let sizes = [16u32, 24, 32, 48, 64, 128, 256];
        let images: Vec<Vec<u8>> = sizes
            .iter()
            .map(|&s| png_of(s, &app_icon_rgba(s)))
            .collect();
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
    fn the_exe_icon_is_the_mark() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/icons/clairlune.ico");
        if std::env::var_os("CLAIRLUNE_WRITE_ICONS").is_some() {
            std::fs::write(&path, ico()).unwrap();
            return;
        }
        assert!(
            std::fs::read(&path).unwrap() == ico(),
            "{} differs from the mark",
            path.display()
        );
    }

    /// The Mac icons (`packaging/icons/<size>.png`) are the mark on its tile:
    /// `CLAIRLUNE_WRITE_ICONS=1 cargo test -p clairlune icon` writes them, and
    /// otherwise this checks the committed ones still match. The 1024 one is
    /// `512@2x.png`: the macOS icon (`.icns`) takes it only as a 512 at 2x.
    #[test]
    fn installer_icons_are_the_mark() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/icons");
        let names = [
            (16u32, "16.png"),
            (32, "32.png"),
            (64, "64.png"),
            (128, "128.png"),
            (256, "256.png"),
            (512, "512.png"),
            (1024, "512@2x.png"),
        ];
        for (size, name) in names {
            let rgba = mac_icon_rgba(size);
            let path = dir.join(name);
            if std::env::var_os("CLAIRLUNE_WRITE_ICONS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(&path, png_of(size, &rgba)).unwrap();
                continue;
            }
            let dec = png::Decoder::new(std::io::BufReader::new(
                std::fs::File::open(&path).unwrap_or_else(|_| panic!("missing {}", path.display())),
            ));
            let mut reader = dec.read_info().unwrap();
            let mut buf = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut buf).unwrap();
            buf.truncate(info.buffer_size());
            assert!(buf == rgba, "{} differs from the mark", path.display());
        }
    }
}
