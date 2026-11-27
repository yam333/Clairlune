//! Image-to-ASCII conversion and custom ASCII wallpaper packaging.
//!
//! Converts an arbitrary RGBA image into:
//! - plain multi-line ASCII/Unicode text (`.txt` / clipboard),
//! - an anti-aliased RGBA image (including transparent background when asked),
//! - a genuine [`ScenePackage`] (with both light and dark themes when using a
//!   Clairlune palette or photo colours, plus optional live weather particles),
//!   playable natively in Still, Live, Library, Archive, Timeline and GIF export.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::package::{CustomTheme, ScenePackage};
use crate::palette::{IndexedPalette, SPRITE_BASE, TRANSPARENT_INDEX, ink_level};
use crate::sign::SIGNATURE;
use clairlune_scene::graph::{ADVANCE_RATIO, FONT_TO_CELL, GridSpec, Lighting, SceneGraph};
use clairlune_scene::particles::{
    CALM_PACE, Emitter, EmitterKind, LOOP_MS, WeatherKind, ember_set, hero_time, weather_set,
};
use clairlune_scene::recipe::{Composition, PalettePick};
use clairlune_scene::{
    Aspect, GENERATOR_VERSION, PaletteFamily, Rgb, SceneRecipe, Theme, TimeOfDay,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Archetype marker stored on custom image-to-ASCII recipes.
pub const PHOTO_ARCHETYPE: &str = "photo";

/// Decoded RGBA8 source image, row-major (`width * height * 4` bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        let want = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| "image dimensions overflow".to_string())?;
        if width == 0 || height == 0 || pixels.len() != want {
            return Err(format!(
                "invalid RGBA image buffer ({} bytes for {width}x{height})",
                pixels.len()
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Downscales the image if its longest side exceeds `max_side`, keeping
    /// saved archive/library copies compact while preserving full ASCII detail.
    pub fn capped(&self, max_side: u32) -> Self {
        let longest = self.width.max(self.height);
        if longest <= max_side || max_side == 0 {
            return self.clone();
        }
        let nw = ((u64::from(self.width) * u64::from(max_side)) / u64::from(longest)).max(1) as u32;
        let nh =
            ((u64::from(self.height) * u64::from(max_side)) / u64::from(longest)).max(1) as u32;
        let mut out = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            let y0 = f64::from(y) * f64::from(self.height) / f64::from(nh);
            let y1 = f64::from(y + 1) * f64::from(self.height) / f64::from(nh);
            for x in 0..nw {
                let x0 = f64::from(x) * f64::from(self.width) / f64::from(nw);
                let x1 = f64::from(x + 1) * f64::from(self.width) / f64::from(nw);
                let s = sample_box(self, x0, y0, x1, y1);
                let idx = ((y * nw + x) * 4) as usize;
                out[idx..idx + 4].copy_from_slice(&[s.r, s.g, s.b, s.a]);
            }
        }
        Self {
            width: nw,
            height: nh,
            pixels: out,
        }
    }
}

/// Line-height / character aspect ratio for standalone `.txt` and PNG output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowDensity {
    /// Matches Clairlune's JetBrains Mono cell geometry (`0.54`).
    #[default]
    Auto,
    /// Tighter terminal spacing (`0.45`).
    Compact,
    /// Classic 1:2 terminal cell (`0.50`).
    Standard,
    /// Looser editor/prose line spacing (`0.62`).
    Tall,
}

impl RowDensity {
    /// Width-to-height ratio of one character cell.
    pub fn cell_aspect(self) -> f64 {
        match self {
            RowDensity::Auto => ADVANCE_RATIO * FONT_TO_CELL,
            RowDensity::Compact => 0.45,
            RowDensity::Standard => 0.50,
            RowDensity::Tall => 0.62,
        }
    }
}

/// Character ramp ordered from heaviest ink to lightest (`' '` last in the UI).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RampPreset {
    #[default]
    Standard,
    Detailed,
    Dither,
    Braille,
    Blocks,
    Lines,
    Kana,
    Shapes,
    Hex,
    Custom,
}

impl RampPreset {
    pub const ALL: [RampPreset; 10] = [
        RampPreset::Standard,
        RampPreset::Detailed,
        RampPreset::Dither,
        RampPreset::Braille,
        RampPreset::Blocks,
        RampPreset::Lines,
        RampPreset::Kana,
        RampPreset::Shapes,
        RampPreset::Hex,
        RampPreset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RampPreset::Standard => "Standard, 10 levels",
            RampPreset::Detailed => "Detailed, 70 levels",
            RampPreset::Dither => "Dithered, 7 levels",
            RampPreset::Braille => "Braille, 2×4 dots",
            RampPreset::Blocks => "Blocks, 5 levels",
            RampPreset::Lines => "Box lines, 8 levels",
            RampPreset::Kana => "Katakana, 10 levels",
            RampPreset::Shapes => "Geometric shapes, 9 levels",
            RampPreset::Hex => "Hex digits, 9 levels",
            RampPreset::Custom => "Custom ramp",
        }
    }

    /// Characters ordered heaviest to lightest (as shown in the UI).
    pub fn heavy_to_light(self, custom: &str) -> Vec<char> {
        let s = match self {
            RampPreset::Standard => "@%#*+=-:. ",
            RampPreset::Detailed => {
                "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/|()1{}[]?-_+~<>i!lI;:,\"^`'. "
            }
            RampPreset::Dither => "■●━▪•· ",
            RampPreset::Braille => "⣿⣷⣯⣟⡿⢿⣻⣽⣾⣶⣦⣤⣄⣀⡀ ",
            RampPreset::Blocks => "█▓▒░ ",
            RampPreset::Lines => "╬╋╪┼═─· ",
            RampPreset::Kana => "ヰモムヌメミニー・ ",
            RampPreset::Shapes => "■●◆▪▫○◦· ",
            RampPreset::Hex => "8BECF731 ",
            RampPreset::Custom => {
                let mut chars: Vec<char> = Vec::new();
                for ch in custom.chars() {
                    if ch != ' '
                        && !ch.is_control()
                        && GlyphAtlas::can_render_char(ch)
                        && !chars.contains(&ch)
                    {
                        chars.push(ch);
                        if chars.len() >= 250 {
                            break;
                        }
                    }
                }
                if chars.is_empty() {
                    chars = "@%#*+=-:.".chars().collect();
                }
                chars.push(' ');
                return chars;
            }
        };
        s.chars().collect()
    }

    /// Characters ordered lightest (`' '`) to heaviest (for density indexing).
    pub fn light_to_heavy(self, custom: &str) -> Vec<char> {
        let mut v = self.heavy_to_light(custom);
        v.reverse();
        v
    }

    /// Matching Clairlune letter set for ambient weather particles.
    pub fn weather_letters(self) -> &'static str {
        match self {
            RampPreset::Braille => "braille",
            RampPreset::Blocks => "blocks",
            RampPreset::Lines => "lines",
            RampPreset::Kana => "kana",
            RampPreset::Shapes => "shapes",
            RampPreset::Hex => "hex",
            _ => "text",
        }
    }
}

/// How the converted ASCII art is coloured and backed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "palette", rename_all = "snake_case")]
pub enum AsciiColorMode {
    /// One of Clairlune's 12 palettes (both light and dark themes).
    Palette(String),
    /// Pure black background (`#000000`) with white characters.
    Black,
    /// Pure white background (`#FFFFFF`) with black characters.
    White,
    /// Transparent background on PNG export; dark polarity by default.
    None,
    /// Each character takes its colour from the source image.
    Photo,
}

impl Default for AsciiColorMode {
    fn default() -> Self {
        AsciiColorMode::Palette("classic".into())
    }
}

/// How an image fits a monitor whose aspect ratio differs from the image's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    Cover,
    #[default]
    Contain,
}

/// Full user-configurable settings for an image-to-ASCII conversion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsciiConfig {
    /// Number of columns across (`20..=320`).
    pub columns: u32,
    #[serde(default)]
    pub row_density: RowDensity,
    #[serde(default)]
    pub ramp: RampPreset,
    #[serde(default = "default_custom_ramp")]
    pub custom_ramp: String,
    /// Contrast percentage (`50..=250`; `100` is neutral, `120` is crisp).
    pub contrast: i32,
    /// Brightness shift (`-100..=100`; `0` is neutral).
    pub brightness: i32,
    /// Invert light/dark polarity.
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub color_mode: AsciiColorMode,
    /// Optional ambient weather id (`"none"`, `"snow"`, `"fireflies"`, ...).
    #[serde(default = "none_weather")]
    pub weather: String,
    #[serde(default)]
    pub fit: FitMode,
}

fn default_custom_ramp() -> String {
    "@%#*+=-:. ".into()
}

fn none_weather() -> String {
    "none".into()
}

impl Default for AsciiConfig {
    fn default() -> Self {
        Self {
            columns: 110,
            row_density: RowDensity::Auto,
            ramp: RampPreset::Standard,
            custom_ramp: default_custom_ramp(),
            contrast: 120,
            brightness: 0,
            invert: false,
            color_mode: AsciiColorMode::Palette("classic".into()),
            weather: none_weather(),
            fit: FitMode::Contain,
        }
    }
}

impl AsciiConfig {
    /// Whether `theme` uses dark-background polarity (characters represent light)
    /// before applying `self.invert`.
    pub fn dark_canvas(&self, theme: Theme) -> bool {
        match &self.color_mode {
            AsciiColorMode::Black => true,
            AsciiColorMode::White => false,
            AsciiColorMode::Palette(_) | AsciiColorMode::Photo | AsciiColorMode::None => {
                theme == Theme::Dark
            }
        }
    }

    /// Whether `self.weather` resolves to an animated particle emitter.
    pub fn has_live_weather(&self) -> bool {
        WeatherKind::from_id(&self.weather).is_some()
    }
}

/// Converted character grid with per-cell colour, opacity and plain text.
#[derive(Clone, Debug)]
pub struct AsciiArt {
    pub cols: u32,
    pub rows: u32,
    pub chars: Vec<char>,
    pub colors: Vec<[u8; 3]>,
    /// Sampled source alpha (`0..=255`) per cell.
    pub alphas: Vec<u8>,
    /// Multi-line text (`\n`-separated rows, trailing spaces trimmed on empty right margins
    /// kept uniform as exact `cols` characters per row so alignment never shifts).
    pub text: String,
}

#[derive(Clone, Copy, Default)]
struct Sample {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

/// Exact area-weighted box average over `[x0, x1] x [y0, y1]` in source pixel space.
fn sample_box(src: &RgbaImage, x0: f64, y0: f64, x1: f64, y1: f64) -> Sample {
    let w = src.width as i32;
    let h = src.height as i32;
    let x0 = x0.clamp(0.0, f64::from(src.width));
    let y0 = y0.clamp(0.0, f64::from(src.height));
    let x1 = x1.clamp(x0, f64::from(src.width));
    let y1 = y1.clamp(y0, f64::from(src.height));
    let ix0 = (x0.floor() as i32).clamp(0, w - 1);
    let iy0 = (y0.floor() as i32).clamp(0, h - 1);
    let ix1 = (x1.ceil() as i32).clamp(ix0 + 1, w);
    let iy1 = (y1.ceil() as i32).clamp(iy0 + 1, h);

    let mut wr = 0.0;
    let mut wg = 0.0;
    let mut wb = 0.0;
    let mut wa = 0.0;
    let mut w_rgb = 0.0;
    let mut w_tot = 0.0;

    for py in iy0..iy1 {
        let oy = (f64::from(py + 1).min(y1) - f64::from(py).max(y0)).max(0.0);
        if oy <= 0.0 {
            continue;
        }
        for px in ix0..ix1 {
            let ox = (f64::from(px + 1).min(x1) - f64::from(px).max(x0)).max(0.0);
            let area = ox * oy;
            if area <= 0.0 {
                continue;
            }
            let idx = ((py as u32 * src.width + px as u32) * 4) as usize;
            let r = f64::from(src.pixels[idx]);
            let g = f64::from(src.pixels[idx + 1]);
            let b = f64::from(src.pixels[idx + 2]);
            let a = f64::from(src.pixels[idx + 3]);
            let alpha_w = area * (a / 255.0);
            wr += r * alpha_w;
            wg += g * alpha_w;
            wb += b * alpha_w;
            w_rgb += alpha_w;
            wa += a * area;
            w_tot += area;
        }
    }
    if w_tot <= 0.0 {
        return Sample::default();
    }
    let a = (wa / w_tot).round().clamp(0.0, 255.0) as u8;
    if w_rgb <= 1e-6 {
        return Sample {
            r: 0,
            g: 0,
            b: 0,
            a,
        };
    }
    Sample {
        r: (wr / w_rgb).round().clamp(0.0, 255.0) as u8,
        g: (wg / w_rgb).round().clamp(0.0, 255.0) as u8,
        b: (wb / w_rgb).round().clamp(0.0, 255.0) as u8,
        a,
    }
}

/// Applies brightness (`-100..=100`), contrast (`50..=250`), alpha compositing
/// over the canvas polarity, and optional inversion to produce ink density in `0..=1`.
fn ink_density(s: Sample, config: &AsciiConfig, dark_canvas: bool) -> f64 {
    if s.a < 16 {
        return 0.0;
    }
    let alpha = f64::from(s.a) / 255.0;
    let bg_luma = if dark_canvas { 0.0 } else { 1.0 };
    let raw_luma =
        (0.2126 * f64::from(s.r) + 0.7152 * f64::from(s.g) + 0.0722 * f64::from(s.b)) / 255.0;
    let luma = raw_luma * alpha + bg_luma * (1.0 - alpha);
    let bright = luma + f64::from(config.brightness.clamp(-100, 100)) / 180.0;
    let contrast = f64::from(config.contrast.clamp(40, 300)) / 100.0;
    let adjusted = ((bright - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
    let polarity = if dark_canvas {
        adjusted
    } else {
        1.0 - adjusted
    };
    if config.invert {
        1.0 - polarity
    } else {
        polarity
    }
}

/// Converts `src` using its natural aspect ratio (for the Studio preview, `.txt`
/// export and standalone PNG export).
pub fn convert_standalone(src: &RgbaImage, config: &AsciiConfig, theme: Theme) -> AsciiArt {
    let cols = config.columns.clamp(20, 320);
    let img_aspect = f64::from(src.height) / f64::from(src.width.max(1));
    let rows = ((f64::from(cols) * img_aspect * config.row_density.cell_aspect()).round() as u32)
        .clamp(8, 320);
    convert_grid(src, config, theme, cols, rows, None)
}

/// Converts `src` onto a fixed `(cols, rows)` wallpaper grid, cropping (`Cover`)
/// or centring (`Contain`) according to `config.fit` and cell pixel aspect `cell_w / cell_h`.
pub fn convert_for_grid(
    src: &RgbaImage,
    config: &AsciiConfig,
    theme: Theme,
    cols: u32,
    rows: u32,
    cell_w: u32,
    cell_h: u32,
) -> AsciiArt {
    let grid_px_w = f64::from(cols * cell_w.max(1));
    let grid_px_h = f64::from(rows * cell_h.max(1));
    convert_grid(src, config, theme, cols, rows, Some((grid_px_w, grid_px_h)))
}

fn convert_grid(
    src: &RgbaImage,
    config: &AsciiConfig,
    theme: Theme,
    cols: u32,
    rows: u32,
    screen_px: Option<(f64, f64)>,
) -> AsciiArt {
    let dark_canvas = config.dark_canvas(theme);
    let ramp = config.ramp.light_to_heavy(&config.custom_ramp);
    let n = (cols * rows) as usize;
    let mut chars = vec![' '; n];
    let mut colors = vec![[0u8; 3]; n];
    let mut alphas = vec![0u8; n];

    let sw = f64::from(src.width);
    let sh = f64::from(src.height);

    // Map cell coordinates [0..cols] x [0..rows] into source pixel space.
    // Either full source (standalone), Cover crop, or Contain letterbox.
    let (crop_x, crop_y, crop_w, crop_h, dst_x0, dst_y0, dst_w, dst_h) = match screen_px {
        None => (0.0, 0.0, sw, sh, 0.0, 0.0, f64::from(cols), f64::from(rows)),
        Some((gw, gh)) => {
            let src_ratio = sw / sh.max(1.0);
            let dst_ratio = gw / gh.max(1.0);
            match config.fit {
                FitMode::Cover => {
                    if src_ratio > dst_ratio {
                        let cw = sh * dst_ratio;
                        (
                            (sw - cw) * 0.5,
                            0.0,
                            cw,
                            sh,
                            0.0,
                            0.0,
                            f64::from(cols),
                            f64::from(rows),
                        )
                    } else {
                        let ch = sw / dst_ratio;
                        (
                            0.0,
                            (sh - ch) * 0.5,
                            sw,
                            ch,
                            0.0,
                            0.0,
                            f64::from(cols),
                            f64::from(rows),
                        )
                    }
                }
                FitMode::Contain => {
                    if src_ratio > dst_ratio {
                        let dh = (f64::from(rows) * (dst_ratio / src_ratio)).max(1.0);
                        (
                            0.0,
                            0.0,
                            sw,
                            sh,
                            0.0,
                            (f64::from(rows) - dh).max(0.0) * 0.5,
                            f64::from(cols),
                            dh,
                        )
                    } else {
                        let dw = (f64::from(cols) * (src_ratio / dst_ratio)).max(1.0);
                        (
                            0.0,
                            0.0,
                            sw,
                            sh,
                            (f64::from(cols) - dw).max(0.0) * 0.5,
                            0.0,
                            dw,
                            f64::from(rows),
                        )
                    }
                }
            }
        }
    };

    let canvas_bg = resolve_family(&config.color_mode, theme).bg_center;
    const BAYER_2X4: [[f64; 2]; 4] = [[-0.07, 0.02], [0.05, -0.04], [-0.02, 0.07], [0.04, -0.05]];
    const BRAILLE_BITS: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

    for row in 0..rows {
        let ry0 = f64::from(row);
        let ry1 = f64::from(row + 1);
        if ry1 <= dst_y0 || ry0 >= dst_y0 + dst_h {
            continue;
        }
        let u_y0 = ((ry0 - dst_y0) / dst_h).clamp(0.0, 1.0);
        let u_y1 = ((ry1 - dst_y0) / dst_h).clamp(0.0, 1.0);
        let sy0 = crop_y + u_y0 * crop_h;
        let sy1 = crop_y + u_y1 * crop_h;

        for col in 0..cols {
            let cx0 = f64::from(col);
            let cx1 = f64::from(col + 1);
            if cx1 <= dst_x0 || cx0 >= dst_x0 + dst_w {
                continue;
            }
            let u_x0 = ((cx0 - dst_x0) / dst_w).clamp(0.0, 1.0);
            let u_x1 = ((cx1 - dst_x0) / dst_w).clamp(0.0, 1.0);
            let sx0 = crop_x + u_x0 * crop_w;
            let sx1 = crop_x + u_x1 * crop_w;

            let idx = (row * cols + col) as usize;
            let cell_sample = sample_box(src, sx0, sy0, sx1, sy1);
            if cell_sample.a == 0 {
                continue;
            }
            let composited = if cell_sample.a < 255 {
                let t = f64::from(cell_sample.a) / 255.0;
                let mixed = canvas_bg.mix(
                    Rgb {
                        r: cell_sample.r,
                        g: cell_sample.g,
                        b: cell_sample.b,
                    },
                    t,
                );
                Sample {
                    r: mixed.r,
                    g: mixed.g,
                    b: mixed.b,
                    a: cell_sample.a,
                }
            } else {
                cell_sample
            };
            colors[idx] = readable_photo_rgb(composited, dark_canvas);

            if config.ramp == RampPreset::Braille {
                // 2x4 sub-cell braille sampling for sharp edges and stippled gradients.
                let mut mask = 0u8;
                let mut sum = 0.0;
                let mut max_sub_a = cell_sample.a;
                for (dy, bit_row) in BRAILLE_BITS.iter().enumerate() {
                    let by0 = sy0 + (sy1 - sy0) * (dy as f64 / 4.0);
                    let by1 = sy0 + (sy1 - sy0) * ((dy + 1) as f64 / 4.0);
                    for (dx, &bit) in bit_row.iter().enumerate() {
                        let bx0 = sx0 + (sx1 - sx0) * (dx as f64 / 2.0);
                        let bx1 = sx0 + (sx1 - sx0) * ((dx + 1) as f64 / 2.0);
                        let sub = sample_box(src, bx0, by0, bx1, by1);
                        max_sub_a = max_sub_a.max(sub.a);
                        let d = ink_density(sub, config, dark_canvas);
                        sum += d;
                        if d + BAYER_2X4[dy][dx] >= 0.34 {
                            mask |= 1 << bit;
                        }
                    }
                }
                if max_sub_a < 16 {
                    continue;
                }
                alphas[idx] = if mask != 0 { max_sub_a } else { cell_sample.a };
                if mask == 0 && sum / 8.0 > 0.14 {
                    mask = 1 << 6;
                    alphas[idx] = max_sub_a;
                }
                chars[idx] = if mask == 0 {
                    ' '
                } else {
                    char::from_u32(0x2800 + u32::from(mask)).unwrap_or(' ')
                };
            } else {
                if cell_sample.a < 16 {
                    continue;
                }
                alphas[idx] = cell_sample.a;
                let d = ink_density(cell_sample, config, dark_canvas);
                let max_i = ramp.len().saturating_sub(1);
                let ri = (d * max_i as f64).round().clamp(0.0, max_i as f64) as usize;
                chars[idx] = ramp[ri];
            }
        }
    }

    let mut text = String::with_capacity(((cols + 1) * rows) as usize);
    for r in 0..rows {
        if r > 0 {
            text.push('\n');
        }
        let start = (r * cols) as usize;
        let line: String = chars[start..start + cols as usize].iter().collect();
        text.push_str(line.trim_end_matches(' '));
    }

    AsciiArt {
        cols,
        rows,
        chars,
        colors,
        alphas,
        text,
    }
}

/// Ensures photo-coloured glyphs keep legible contrast against the chosen canvas.
fn readable_photo_rgb(s: Sample, dark_canvas: bool) -> [u8; 3] {
    let luma = (u32::from(s.r) * 2126 + u32::from(s.g) * 7152 + u32::from(s.b) * 722) / 10_000;
    if dark_canvas && luma < 48 {
        let lift = (48 - luma) as u8;
        [
            s.r.saturating_add(lift),
            s.g.saturating_add(lift),
            s.b.saturating_add(lift),
        ]
    } else if !dark_canvas && luma > 212 {
        let drop = (luma - 212) as u8;
        [
            s.r.saturating_sub(drop),
            s.g.saturating_sub(drop),
            s.b.saturating_sub(drop),
        ]
    } else {
        [s.r, s.g, s.b]
    }
}

/// Resolves the `PaletteFamily` for a `(color_mode, theme)` pair.
pub fn resolve_family(mode: &AsciiColorMode, theme: Theme) -> PaletteFamily {
    match mode {
        AsciiColorMode::Palette(set) => clairlune_scene::palette::families_in(set, theme)
            .next()
            .copied()
            .or_else(|| {
                clairlune_scene::palette::families_for(theme)
                    .next()
                    .copied()
            })
            .expect("classic palette exists"),
        AsciiColorMode::Black => PaletteFamily {
            id: "ascii_black",
            set: "classic",
            theme: Theme::Dark,
            bg_edge: Rgb::hex(0x000000),
            bg_center: Rgb::hex(0x000000),
            ink: Rgb::hex(0xF2F2F2),
            accent: Rgb::hex(0xFF9646),
            ember: Rgb::hex(0xFFB066),
            glow_strength: 0.14,
            tint_strength: 0.50,
        },
        AsciiColorMode::White => PaletteFamily {
            id: "ascii_white",
            set: "classic",
            theme: Theme::Light,
            bg_edge: Rgb::hex(0xFFFFFF),
            bg_center: Rgb::hex(0xFFFFFF),
            ink: Rgb::hex(0x141414),
            accent: Rgb::hex(0xD9652B),
            ember: Rgb::hex(0xCF5320),
            glow_strength: 0.10,
            tint_strength: 0.45,
        },
        AsciiColorMode::None => match theme {
            Theme::Dark => PaletteFamily {
                id: "ascii_black",
                set: "classic",
                theme: Theme::Dark,
                bg_edge: Rgb::hex(0x000000),
                bg_center: Rgb::hex(0x000000),
                ink: Rgb::hex(0xF2F2F2),
                accent: Rgb::hex(0xFF9646),
                ember: Rgb::hex(0xFFB066),
                glow_strength: 0.14,
                tint_strength: 0.50,
            },
            Theme::Light => PaletteFamily {
                id: "ascii_white",
                set: "classic",
                theme: Theme::Light,
                bg_edge: Rgb::hex(0xFFFFFF),
                bg_center: Rgb::hex(0xFFFFFF),
                ink: Rgb::hex(0x141414),
                accent: Rgb::hex(0xD9652B),
                ember: Rgb::hex(0xCF5320),
                glow_strength: 0.10,
                tint_strength: 0.45,
            },
        },
        AsciiColorMode::Photo => match theme {
            Theme::Dark => PaletteFamily {
                id: "photo_dark",
                set: "classic",
                theme: Theme::Dark,
                bg_edge: Rgb::hex(0x0B0C10),
                bg_center: Rgb::hex(0x13151C),
                ink: Rgb::hex(0xE9E5D7),
                accent: Rgb::hex(0xFF9646),
                ember: Rgb::hex(0xFFA65C),
                glow_strength: 0.16,
                tint_strength: 0.55,
            },
            Theme::Light => PaletteFamily {
                id: "photo_light",
                set: "classic",
                theme: Theme::Light,
                bg_edge: Rgb::hex(0xEAE6DC),
                bg_center: Rgb::hex(0xF6F3EC),
                ink: Rgb::hex(0x1C1A17),
                accent: Rgb::hex(0xD9652B),
                ember: Rgb::hex(0xCF5320),
                glow_strength: 0.10,
                tint_strength: 0.50,
            },
        },
    }
}

/// Rasterizes every unique character in `chars` in chunks of `<= 200` glyphs,
/// returning `(per_cell_glyph_id, contiguous_masks)` with `0` reserved for `' '`.
fn rasterize_unique_chars(
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    font_px: f64,
) -> (Vec<u16>, Vec<u8>) {
    let mut unique: Vec<char> = vec![' '];
    let mut map: HashMap<char, u16> = HashMap::new();
    map.insert(' ', 0);
    let mut ids = Vec::with_capacity(chars.len());
    for &c in chars {
        let id = if let Some(&id) = map.get(&c) {
            id
        } else {
            let id = unique.len() as u16;
            unique.push(c);
            map.insert(c, id);
            id
        };
        ids.push(id);
    }
    let cell_len = (cell_w * cell_h) as usize;
    let mut all_masks = vec![0u8; unique.len() * cell_len];
    for (chunk_idx, chunk) in unique.chunks(200).enumerate() {
        let atlas = GlyphAtlas::for_char_set(chunk, cell_w, cell_h, font_px);
        for (local_i, _) in chunk.iter().enumerate() {
            let global_i = chunk_idx * 200 + local_i;
            all_masks[global_i * cell_len..(global_i + 1) * cell_len]
                .copy_from_slice(atlas.mask(local_i as u8));
        }
    }
    (ids, all_masks)
}

/// Renders `art` to an RGBA8 image (`width, height, pixels`), supporting
/// transparent backgrounds (`AsciiColorMode::None`), photo colours, palettes,
/// and optional corner signature on opaque exports.
pub fn render_rgba(
    art: &AsciiArt,
    config: &AsciiConfig,
    theme: Theme,
    cell_h: u32,
    sign: bool,
) -> (u32, u32, Vec<u8>) {
    let cell_h = cell_h.max(8);
    let cell_w = ((f64::from(cell_h) * FONT_TO_CELL * ADVANCE_RATIO).round() as u32).max(4);
    let transparent = config.color_mode == AsciiColorMode::None;
    let pad_x = cell_w * 2;
    let pad_y = cell_h;
    let sig_pad_bottom = if sign && !transparent {
        cell_h.max(16) + 8
    } else {
        0
    };
    let width = art.cols * cell_w + pad_x * 2;
    let height = art.rows * cell_h + pad_y * 2 + sig_pad_bottom;

    let family = resolve_family(&config.color_mode, theme);
    let lighting = Lighting {
        vignette_cx: f64::from(width) * 0.5,
        vignette_cy: f64::from(height) * 0.5,
        vignette_rx: f64::from(width) * 0.78,
        vignette_ry: f64::from(height) * 0.78,
        glow: None,
    };

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    if !transparent {
        let flat = matches!(
            config.color_mode,
            AsciiColorMode::Black | AsciiColorMode::White
        );
        for y in 0..height {
            let rt = lighting.row_terms(y);
            for x in 0..width {
                let bg = if flat {
                    family.bg_center
                } else {
                    let b = lighting.band_in_row(&rt, x).min(3);
                    family.bg_edge.mix(family.bg_center, f64::from(b) / 3.0)
                };
                let o = ((y * width + x) * 4) as usize;
                rgba[o] = bg.r;
                rgba[o + 1] = bg.g;
                rgba[o + 2] = bg.b;
                rgba[o + 3] = 255;
            }
        }
    }

    let font_px = f64::from(cell_h) * FONT_TO_CELL;
    let (glyph_ids, all_masks) = rasterize_unique_chars(&art.chars, cell_w, cell_h, font_px);
    let cell_len = (cell_w * cell_h) as usize;

    for row in 0..art.rows {
        for col in 0..art.cols {
            let ci = (row * art.cols + col) as usize;
            let gid = glyph_ids[ci] as usize;
            if gid == 0 {
                continue;
            }
            let mask = &all_masks[gid * cell_len..(gid + 1) * cell_len];
            let ink = if config.color_mode == AsciiColorMode::Photo {
                let [r, g, b] = art.colors[ci];
                Rgb { r, g, b }
            } else {
                family.ink
            };
            let x0 = pad_x + col * cell_w;
            let y0 = pad_y + row * cell_h;
            for my in 0..cell_h {
                for mx in 0..cell_w {
                    let a = mask[(my * cell_w + mx) as usize];
                    if a == 0 {
                        continue;
                    }
                    let o = (((y0 + my) * width + (x0 + mx)) * 4) as usize;
                    if transparent {
                        let scaled_a =
                            ((u16::from(a) * u16::from(art.alphas[ci]) + 127) / 255) as u8;
                        if scaled_a == 0 {
                            continue;
                        }
                        rgba[o] = ink.r;
                        rgba[o + 1] = ink.g;
                        rgba[o + 2] = ink.b;
                        rgba[o + 3] = scaled_a;
                    } else {
                        let t = f64::from(a) / 255.0;
                        let cur = Rgb {
                            r: rgba[o],
                            g: rgba[o + 1],
                            b: rgba[o + 2],
                        };
                        let m = cur.mix(ink, t);
                        rgba[o] = m.r;
                        rgba[o + 1] = m.g;
                        rgba[o + 2] = m.b;
                        rgba[o + 3] = 255;
                    }
                }
            }
        }
    }

    if sign && !transparent {
        stamp_rgba_signature(&mut rgba, width, height, family.ink);
    }

    (width, height, rgba)
}

fn stamp_rgba_signature(rgba: &mut [u8], width: u32, height: u32, ink: Rgb) {
    let spec = clairlune_scene::grid_spec(width, height);
    let chars: Vec<char> = SIGNATURE.chars().collect();
    let n = chars.len() as u32;
    if spec.cols < 2 * (n + 3) || spec.rows < 6 {
        return;
    }
    let Ok(atlas) = GlyphAtlas::with_chars(
        crate::atlas::FONT_DATA,
        &chars,
        spec.cell_w,
        spec.cell_h,
        spec.font_px,
    ) else {
        return;
    };
    let x0 = spec.origin_x + (spec.cols - n - 2) * spec.cell_w;
    let y0 = spec.origin_y + (spec.rows - 2) * spec.cell_h;
    for c in 0..n {
        let mask = atlas.mask(c as u8);
        for my in 0..spec.cell_h {
            for mx in 0..spec.cell_w {
                let a = mask[(my * spec.cell_w + mx) as usize];
                if a == 0 {
                    continue;
                }
                let px = x0 + c * spec.cell_w + mx;
                let py = y0 + my;
                if px >= width || py >= height {
                    continue;
                }
                let o = ((py * width + px) * 4) as usize;
                let cur = Rgb {
                    r: rgba[o],
                    g: rgba[o + 1],
                    b: rgba[o + 2],
                };
                let m = cur.mix(ink, (f64::from(a) / 255.0) * 0.65);
                rgba[o] = m.r;
                rgba[o + 1] = m.g;
                rgba[o + 2] = m.b;
            }
        }
    }
}

/// Encodes an RGBA8 image buffer as a PNG.
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Builds a frozen [`ScenePackage`] at `(width, height)` for a user image,
/// complete with both themes (unless fixed to Black or White) and optional
/// live weather particles.
pub fn build_photo_package(
    src: &RgbaImage,
    config: &AsciiConfig,
    width: u32,
    height: u32,
    seed: u64,
) -> Result<ScenePackage, String> {
    if width < 160 || height < 90 {
        return Err(format!("{width}x{height} is too small (minimum 160x90)"));
    }

    let wanted_cols = config.columns.clamp(24, 320);
    let max_cell_w = (width / wanted_cols).max(3);
    let cell_h_from_cols =
        ((f64::from(max_cell_w) / (ADVANCE_RATIO * FONT_TO_CELL)).round() as u32).max(6);
    let font_px = f64::from(cell_h_from_cols) * FONT_TO_CELL;
    let cell_w = ((font_px * ADVANCE_RATIO).round() as u32)
        .clamp(3, width)
        .min(max_cell_w);
    let cell_h = cell_h_from_cols.min(height);
    let cols = (width / cell_w).max(1);
    let rows = (height / cell_h).max(1);
    let ascii_grid = GridSpec {
        cols,
        rows,
        cell_w,
        cell_h,
        origin_x: (width - cols * cell_w) / 2,
        origin_y: (height - rows * cell_h) / 2,
        font_px,
    };
    // Weather particles use the screen's standard scene grid so snowflakes/rain
    // stay crisp regardless of how wide or narrow `config.columns` is set.
    let scene_grid = clairlune_scene::grid_spec(width, height);

    let themes_to_build: &[Theme] = match config.color_mode {
        AsciiColorMode::Black => &[Theme::Dark],
        AsciiColorMode::White => &[Theme::Light],
        AsciiColorMode::Palette(_) | AsciiColorMode::None | AsciiColorMode::Photo => {
            &[Theme::Dark, Theme::Light]
        }
    };

    let letters = config.ramp.weather_letters();
    let weather_atlas = GlyphAtlas::for_letters(&scene_grid, letters);
    let embers = ember_set(seed, CALM_PACE);
    let t_hero_ms = hero_time(&embers);
    let aspect = f64::from(width) / f64::from(height);
    let unit = f64::from(width.min(height));

    let mut emitters = Vec::new();
    if let Some(kind) = WeatherKind::from_id(&config.weather) {
        emitters.push(Emitter {
            kind: EmitterKind::Weather(kind),
            origin_x: 0.0,
            origin_y: 0.0,
            unit,
            intensity: 1.0,
            embers: Vec::new(),
            drops: weather_set(kind, seed, aspect, CALM_PACE),
            size: (f64::from(width), f64::from(height)),
            pace: CALM_PACE,
        });
    }

    let mut custom_themes = Vec::with_capacity(themes_to_build.len());
    for &theme in themes_to_build {
        let tod = match theme {
            Theme::Dark => TimeOfDay::Night,
            Theme::Light => TimeOfDay::Morning,
        };
        let family = resolve_family(&config.color_mode, theme);
        let art = convert_for_grid(src, config, theme, cols, rows, cell_w, cell_h);
        let lighting = Lighting {
            vignette_cx: f64::from(width) * 0.5,
            vignette_cy: f64::from(height) * 0.5,
            vignette_rx: f64::from(width) * 0.78,
            vignette_ry: f64::from(height) * 0.78,
            glow: None,
        };

        let graph = SceneGraph {
            width,
            height,
            theme,
            tod,
            buddy: false,
            seed,
            family,
            grid: scene_grid,
            cells: Vec::new(),
            lighting,
            activity: None,
            props: Vec::new(),
            unattended: Vec::new(),
            character: None,
            emitters: emitters.clone(),
            ground_y: height as i32,
            t_hero_ms,
            loop_ms: LOOP_MS,
            pace: CALM_PACE,
            generator: GENERATOR_VERSION,
            letters: if letters == "text" {
                String::new()
            } else {
                letters.to_string()
            },
            water: None,
            journey: None,
            zoom: 1.0,
        };

        let (background, palette) = bake_ascii_theme(&art, config, &graph, &ascii_grid);
        custom_themes.push(CustomTheme {
            theme,
            graph,
            palette,
            atlas: weather_atlas.clone(),
            background,
            hero_ms: t_hero_ms,
        });
    }

    let (light_family, dark_family) = match &config.color_mode {
        AsciiColorMode::Palette(set) => {
            let l = clairlune_scene::palette::families_in(set, Theme::Light)
                .next()
                .map_or("paper_day", |f| f.id);
            let d = clairlune_scene::palette::families_in(set, Theme::Dark)
                .next()
                .map_or("ember_night", |f| f.id);
            (l.to_string(), d.to_string())
        }
        _ => ("paper_day".to_string(), "ember_night".to_string()),
    };

    let recipe = SceneRecipe {
        generator_version: GENERATOR_VERSION,
        seed,
        aspect: Aspect::from_size(width, height),
        content_ctx_hash: format!("{seed:016x}"),
        activity: None,
        archetype: PHOTO_ARCHETYPE.into(),
        light_tod: themes_to_build
            .contains(&Theme::Light)
            .then_some(TimeOfDay::Morning),
        dark_tod: themes_to_build
            .contains(&Theme::Dark)
            .then_some(TimeOfDay::Night),
        platform: "none".into(),
        props: Vec::new(),
        weather: config.weather.clone(),
        composition: Composition {
            horizon_pm: 800,
            focus_x_pm: 500,
            gap_pm: 500,
            shape_pow_pm: 1000,
            platform_margin_pm: 0,
            cliff_layers: 0,
            prop_left: false,
            motion_speed_cc: 0,
            pace_half_pm: 0,
            fly_lift_pm: 0,
            journey_ms: 0,
            journey_pace_c: 0,
            landmark: String::new(),
        },
        palette: PalettePick {
            light: light_family,
            dark: dark_family,
        },
        look: "line".into(),
        contrast: "normal".into(),
        zoom: "close".into(),
        letters: letters.into(),
    };

    ScenePackage::from_custom(recipe, width, height, custom_themes).map_err(|e| e.to_string())
}

fn bake_ascii_theme(
    art: &AsciiArt,
    config: &AsciiConfig,
    graph: &SceneGraph,
    g: &GridSpec,
) -> (IndexedImage, IndexedPalette) {
    let mut img = IndexedImage::new(graph.width, graph.height);
    let w = graph.width as usize;
    let flat_bg = matches!(
        config.color_mode,
        AsciiColorMode::Black | AsciiColorMode::White | AsciiColorMode::None
    );
    if flat_bg {
        img.pixels.fill(3); // uniform bg_center band
    } else {
        for (y, row) in img.pixels.chunks_exact_mut(w).enumerate() {
            graph.lighting.fill_row(y as u32, row);
        }
    }

    let (cw, ch) = (g.cell_w as usize, g.cell_h as usize);
    let cell_len = cw * ch;
    let (glyph_ids, all_masks) = rasterize_unique_chars(&art.chars, g.cell_w, g.cell_h, g.font_px);

    if config.color_mode == AsciiColorMode::Photo {
        let (palette, cell_slots) = build_photo_palette(graph, art);
        for row in 0..g.rows {
            for col in 0..g.cols {
                let ci = (row * g.cols + col) as usize;
                let gid = glyph_ids[ci] as usize;
                if gid == 0 {
                    continue;
                }
                let base_slot = cell_slots[ci];
                let mask = &all_masks[gid * cell_len..(gid + 1) * cell_len];
                let x0 = g.origin_x as usize + col as usize * cw;
                let y0 = g.origin_y as usize + row as usize * ch;
                for my in 0..ch {
                    let line = (y0 + my) * w + x0;
                    for mx in 0..cw {
                        let a = mask[my * cw + mx];
                        // 3 anti-aliasing levels per quantized colour: faint, medium, full.
                        let step = if a < 48 {
                            continue;
                        } else if a < 128 {
                            0
                        } else if a < 205 {
                            1
                        } else {
                            2
                        };
                        img.pixels[line + mx] = base_slot + step;
                    }
                }
            }
        }
        return (img, palette);
    }

    let palette = IndexedPalette::for_graph(graph);
    for row in 0..g.rows {
        for col in 0..g.cols {
            let ci = (row * g.cols + col) as usize;
            let gid = glyph_ids[ci] as usize;
            if gid == 0 {
                continue;
            }
            let mask = &all_masks[gid * cell_len..(gid + 1) * cell_len];
            let x0 = g.origin_x as usize + col as usize * cw;
            let y0 = g.origin_y as usize + row as usize * ch;
            for my in 0..ch {
                let line = (y0 + my) * w + x0;
                for mx in 0..cw {
                    let a = mask[my * cw + mx];
                    if a == 0 {
                        continue;
                    }
                    let level = ink_level(a, 255);
                    if level > 0 {
                        let band = img.pixels[line + mx];
                        img.pixels[line + mx] = IndexedPalette::ink(band, level);
                    }
                }
            }
        }
    }
    (img, palette)
}

/// Builds an `IndexedPalette` whose `0..104` entries keep standard background,
/// ink and ember ramps (for vignette, live weather and signatures) and whose
/// `104..248` entries hold up to 48 median-cut photo colours × 3 AA blend steps.
fn build_photo_palette(graph: &SceneGraph, art: &AsciiArt) -> (IndexedPalette, Vec<u8>) {
    let base = IndexedPalette::for_graph(graph);
    let mut colors = base.colors()[..SPRITE_BASE as usize].to_vec();

    let active: Vec<[u8; 3]> = art
        .chars
        .iter()
        .zip(&art.colors)
        .filter(|(ch, _)| **ch != ' ')
        .map(|(_, &rgb)| rgb)
        .collect();

    let centroids = median_cut(&active, 48);
    let bg = graph.family.bg_center;
    for &[r, g, b] in &centroids {
        let target = Rgb { r, g, b };
        for t in [0.38, 0.70, 1.0] {
            colors.push(bg.mix(target, t));
        }
    }
    debug_assert!(colors.len() < TRANSPARENT_INDEX as usize);

    let cell_slots = art
        .colors
        .iter()
        .map(|rgb| {
            let best = centroids
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| color_dist2(*rgb, **c))
                .map_or(0, |(i, _)| i);
            SPRITE_BASE + (best as u8) * 3
        })
        .collect();

    (IndexedPalette::from_colors(colors), cell_slots)
}

fn color_dist2(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = i32::from(a[0]) - i32::from(b[0]);
    let dg = i32::from(a[1]) - i32::from(b[1]);
    let db = i32::from(a[2]) - i32::from(b[2]);
    (dr * dr * 2 + dg * dg * 4 + db * db) as u32
}

/// Deterministic median-cut colour quantizer producing `1..=max_colors` RGB centroids.
fn median_cut(samples: &[[u8; 3]], max_colors: usize) -> Vec<[u8; 3]> {
    if samples.is_empty() {
        return vec![[220, 220, 220]];
    }
    let mut buckets: Vec<Vec<[u8; 3]>> = vec![samples.to_vec()];
    while buckets.len() < max_colors {
        let Some((split_idx, axis)) = buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() >= 2)
            .filter_map(|(i, b)| {
                let (mut rmin, mut rmax) = (255u8, 0u8);
                let (mut gmin, mut gmax) = (255u8, 0u8);
                let (mut bmin, mut bmax) = (255u8, 0u8);
                for c in b {
                    rmin = rmin.min(c[0]);
                    rmax = rmax.max(c[0]);
                    gmin = gmin.min(c[1]);
                    gmax = gmax.max(c[1]);
                    bmin = bmin.min(c[2]);
                    bmax = bmax.max(c[2]);
                }
                let ranges = [rmax - rmin, gmax - gmin, bmax - bmin];
                let (axis, &span) = ranges.iter().enumerate().max_by_key(|(_, s)| *s)?;
                (span > 0).then_some((i, axis, u32::from(span) * (b.len() as u32).ilog2().max(1)))
            })
            .max_by_key(|&(_, _, score)| score)
            .map(|(i, axis, _)| (i, axis))
        else {
            break;
        };

        let mut bucket = buckets.swap_remove(split_idx);
        bucket.sort_by_key(|c| (c[axis], c[0], c[1], c[2]));
        let mut mid = bucket.len() / 2;
        let min_v = bucket[0][axis];
        let max_v = bucket[bucket.len() - 1][axis];
        if bucket[0][axis] == bucket[mid][axis] || bucket[mid - 1][axis] == max_v {
            let thr = ((u16::from(min_v) + u16::from(max_v)) / 2) as u8;
            mid = bucket
                .partition_point(|c| c[axis] <= thr)
                .clamp(1, bucket.len() - 1);
        }
        let right = bucket.split_off(mid);
        buckets.push(bucket);
        buckets.push(right);
    }

    buckets
        .into_iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n = b.len() as u64;
            let (sr, sg, sb) = b.iter().fold((0u64, 0u64, 0u64), |(r, g, bl), c| {
                (
                    r + u64::from(c[0]),
                    g + u64::from(c[1]),
                    bl + u64::from(c[2]),
                )
            });
            [
                ((sr + n / 2) / n) as u8,
                ((sg + n / 2) / n) as u8,
                ((sb + n / 2) / n) as u8,
            ]
        })
        .collect()
}

/// Number of built-in procedural showcase images for the Studio's Inspire button.
pub const INSPIRE_COUNT: usize = 12;

/// Generates one of the built-in showcase images (`360 x 240` RGBA) in pure Rust.
pub fn inspire_sample(index: usize, buddy: bool) -> (&'static str, RgbaImage) {
    let (w, h) = (360u32, 240u32);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let which = index % INSPIRE_COUNT;
    let title = match which {
        0 => "Crescent & peaks",
        1 => "Sunset horizon",
        2 => "Faceted ring",
        3 if buddy => "Clawd by starlight",
        3 => "Paper lantern",
        4 => "Spiral nebula",
        5 => "Ringed planet",
        6 => "Lighthouse beam",
        7 => "Torii & harvest moon",
        8 => "Great wave",
        9 => "Jellyfish bloom",
        10 => "Aurora pines",
        _ => "Origami crane",
    };

    for y in 0..h {
        let ny = f64::from(y) / f64::from(h);
        for x in 0..w {
            let nx = f64::from(x) / f64::from(w);
            let [r, g, b, a] = match which {
                0 => opaque(sample_crescent_peaks(nx, ny)),
                1 => opaque(sample_sunset(nx, ny)),
                2 => opaque(sample_torus(nx, ny)),
                3 if buddy => opaque(sample_clawd_card(nx, ny)),
                3 => opaque(sample_paper_lantern(nx, ny)),
                4 => opaque(sample_nebula(nx, ny)),
                5 => opaque(sample_ringed_planet(nx, ny)),
                6 => opaque(sample_lighthouse(nx, ny)),
                7 => opaque(sample_torii_moon(nx, ny)),
                8 => opaque(sample_great_wave(nx, ny)),
                9 => opaque(sample_jellyfish(nx, ny)),
                10 => opaque(sample_aurora_pines(nx, ny)),
                _ => sample_origami_crane(nx, ny),
            };
            let o = ((y * w + x) * 4) as usize;
            px[o] = r;
            px[o + 1] = g;
            px[o + 2] = b;
            px[o + 3] = a;
        }
    }
    (
        title,
        RgbaImage {
            width: w,
            height: h,
            pixels: px,
        },
    )
}

#[inline]
fn opaque(rgb: [u8; 3]) -> [u8; 4] {
    [rgb[0], rgb[1], rgb[2], 255]
}

fn star_spark(nx: f64, ny: f64, seed: u32) -> u8 {
    let gx = (nx * 90.0) as u32;
    let gy = (ny * 60.0) as u32;
    let fx = (nx * 90.0).fract() - 0.5;
    let fy = (ny * 60.0).fract() - 0.5;
    let mut h = gx
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(gy.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE35));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    if !h.is_multiple_of(97) {
        return 0;
    }
    let d = (fx * fx + fy * fy).sqrt();
    if d < 0.34 {
        ((1.0 - d / 0.34) * (150.0 + f64::from(h % 95))) as u8
    } else {
        0
    }
}

fn sample_crescent_peaks(nx: f64, ny: f64) -> [u8; 3] {
    // Sky gradient + crescent moon + twin alpine ridges + lake reflection.
    let water_y = 0.72;
    let (ux, uy, mir) = if ny > water_y {
        let ripple = ((ny - water_y) * 190.0).sin() * 0.008;
        (
            (nx + ripple).clamp(0.0, 1.0),
            (water_y - (ny - water_y) * 0.88).clamp(0.0, water_y),
            true,
        )
    } else {
        (nx, ny, false)
    };

    // Crescent moon at (0.66, 0.27)
    let dx = (ux - 0.66) * 1.5;
    let dy = uy - 0.27;
    let d_outer = (dx * dx + dy * dy).sqrt();
    let d_cut = ((dx - 0.065) * (dx - 0.065) + (dy + 0.025) * (dy + 0.025)).sqrt();

    let ridge_far = 0.56
        - 0.19 * (1.0 - ((ux - 0.28).abs() * 3.4).min(1.0))
        - 0.14 * (1.0 - ((ux - 0.74).abs() * 3.8).min(1.0))
        + 0.02 * (ux * 38.0).sin();
    let ridge_near =
        0.68 - 0.26 * (1.0 - ((ux - 0.42).abs() * 2.8).min(1.0)) - 0.03 * (ux * 29.0).cos();

    let mut rgb = if uy > ridge_near {
        let slope = if ux < 0.42 { 0.52 } else { 0.88 };
        let v = (52.0 + slope * 78.0 * (1.0 - (uy - ridge_near) * 1.9).clamp(0.2, 1.0)) as u8;
        [v.saturating_sub(12), v, v.saturating_add(18)]
    } else if uy > ridge_far {
        let v = (34.0 + (0.72 - uy) * 95.0) as u8;
        [v.saturating_sub(8), v, v.saturating_add(24)]
    } else if d_outer < 0.145 && d_cut > 0.125 {
        [250, 240, 206]
    } else {
        let glow = (1.0 - (d_outer / 0.28).min(1.0)).powi(3) * 26.0;
        let base = 6.0 + uy * 18.0 + glow;
        [
            base as u8,
            (base * 1.08) as u8,
            (base * 1.35).min(255.0) as u8,
        ]
    };
    if mir {
        rgb = [
            (u16::from(rgb[0]) * 6 / 10) as u8,
            (u16::from(rgb[1]) * 7 / 10) as u8,
            (u16::from(rgb[2]) * 8 / 10) as u8,
        ];
    }
    rgb
}

fn sample_sunset(nx: f64, ny: f64) -> [u8; 3] {
    let horizon = 0.62;
    let dx = (nx - 0.5) * 1.5;
    let dy = ny - 0.44;
    let d = (dx * dx + dy * dy).sqrt();
    if ny < horizon {
        if d < 0.24 {
            // Striped retro sun
            let band = ((ny - 0.34) * 55.0) as i32;
            if ny > 0.38 && band % 3 == 0 {
                return [38, 18, 52];
            }
            let t = ((ny - 0.20) / 0.30).clamp(0.0, 1.0);
            return [252, (225.0 - t * 115.0) as u8, (95.0 + t * 45.0) as u8];
        }
        let hill = horizon - 0.08 * (nx * 9.0).sin().abs() - 0.04 * (nx * 21.0 + 1.0).sin().abs();
        if ny > hill {
            return [24, 16, 36];
        }
        let t = (ny / horizon).clamp(0.0, 1.0);
        [
            (28.0 + t * 140.0) as u8,
            (20.0 + t * 62.0) as u8,
            (58.0 + t * 55.0) as u8,
        ]
    } else {
        let depth = (ny - horizon) / (1.0 - horizon);
        let sun_col = (1.0 - ((nx - 0.5).abs() * (3.2 + depth * 3.0)).min(1.0)).max(0.0);
        let wave = ((depth * 85.0 + nx * 18.0).sin() * 0.5 + 0.5) * sun_col;
        [
            (22.0 + wave * 220.0) as u8,
            (18.0 + wave * 120.0) as u8,
            (44.0 + wave * 65.0 + (1.0 - depth) * 28.0) as u8,
        ]
    }
}

fn sample_torus(nx: f64, ny: f64) -> [u8; 3] {
    let u = (nx - 0.5) * 2.4;
    let v = (ny - 0.5) * 1.65;
    // Tilted 3D ring normal approximation
    let r = (u * u + (v * 1.28) * (v * 1.28)).sqrt();
    let major = 0.62;
    let minor = 0.25;
    let dr = r - major;
    let inside = minor * minor - dr * dr;
    if inside <= 0.0 {
        let vignette = (1.0 - (r * 0.6).min(1.0)) * 22.0;
        return [
            vignette as u8,
            (vignette * 1.05) as u8,
            (vignette * 1.25) as u8,
        ];
    }
    let nz = inside.sqrt() / minor;
    let inv_r = if r > 1e-4 { 1.0 / r } else { 0.0 };
    let nx3 = (dr / minor) * (u * inv_r);
    let ny3 = (dr / minor) * (v * 1.28 * inv_r);
    let diff = (-0.52 * nx3 - 0.58 * ny3 + 0.62 * nz).max(0.06);
    let spec = diff.powi(6) * 0.55;
    let l = (diff * 0.85 + spec).clamp(0.0, 1.0);
    [
        (30.0 + l * 220.0) as u8,
        (38.0 + l * 198.0) as u8,
        (54.0 + l * 175.0) as u8,
    ]
}

fn sample_clawd_card(nx: f64, ny: f64) -> [u8; 3] {
    // 3D-shaded pixel Clawd beside a crackling campfire under a starry sky.
    let gx = (nx * 60.0).floor() as i32 - 30;
    let gy = (ny * 40.0).floor() as i32 - 20;
    let cx = -6;
    let cy = 2;
    let lx = gx - cx;
    let ly = gy - cy;

    let corner_clip = lx.abs() == 8 && ly == -6;
    let in_body = (-8..=8).contains(&lx) && (-6..=3).contains(&ly) && !corner_clip;
    let in_arms = ((-11..=-9).contains(&lx) || (9..=11).contains(&lx)) && (-2..=1).contains(&ly);
    let in_legs = ([-7, -6, -3, -2, 2, 3, 6, 7].contains(&lx)) && (4..=7).contains(&ly);
    let in_eyes = ([-4, -3, 3, 4].contains(&lx)) && (-3..=-2).contains(&ly);

    if in_eyes {
        return [14, 12, 16];
    }
    if in_body || in_arms {
        // Warm directional firelight from the campfire on the right (+lx) plus top rim light.
        let fire_lit = ((f64::from(lx + 11) / 22.0) * 0.65
            + (1.0 - f64::from(ly + 6) / 11.0) * 0.35)
            .clamp(0.0, 1.0);
        return [
            (132.0 + fire_lit * 120.0) as u8,
            (62.0 + fire_lit * 108.0) as u8,
            (48.0 + fire_lit * 72.0) as u8,
        ];
    }
    if in_legs {
        let lit = if lx > 0 { 28 } else { 0 };
        return [156 + lit, 76 + lit / 2, 58];
    }

    // Campfire at (0.68, 0.73)
    let fx = (nx - 0.68) * 1.5;
    let fy = 0.74 - ny;
    if (-0.025..=0.018).contains(&fy) && fx.abs() < 0.085 {
        return [96, 56, 36]; // crossed logs
    }
    if (0.0..=0.19).contains(&fy) {
        let sway = (fy * 34.0).sin() * 0.012;
        let half_w = 0.068 * (1.0 - (fy / 0.19).powf(0.85));
        let df = (fx - sway).abs();
        if df < half_w {
            let core = (1.0 - df / half_w.max(1e-4)) * (1.0 - fy / 0.22);
            return [
                255,
                (125.0 + core * 125.0).min(255.0) as u8,
                (40.0 + core * 165.0).min(255.0) as u8,
            ];
        }
    }

    // Terraced mossy ledge lit around the fire
    let ground = 0.74 + 0.025 * (nx * 9.0 - 1.2).cos();
    if ny >= ground {
        let fire_dist = ((nx - 0.68).abs() * 2.1 + (ny - ground) * 2.6).min(1.0);
        let warm = (1.0 - fire_dist).powi(2);
        return [
            (32.0 + warm * 145.0) as u8,
            (38.0 + warm * 92.0) as u8,
            (46.0 + warm * 38.0) as u8,
        ];
    }

    let d_fire = (((nx - 0.68) * 1.4).powi(2) + (ny - 0.68).powi(2)).sqrt();
    let ember_halo = (1.0 - (d_fire / 0.26).min(1.0)).powi(3) * 28.0;
    let s = star_spark(nx, ny, 41);
    [
        (8.0 + ember_halo) as u8 | s,
        (10.0 + ember_halo * 0.6) as u8 | s,
        (16.0 + ember_halo * 0.3) as u8 | s,
    ]
}

fn sample_paper_lantern(nx: f64, ny: f64) -> [u8; 3] {
    // Glowing ribbed chochin lantern hanging from a pine branch.
    let branch_y = 0.11 + 0.08 * (1.0 - nx).powi(2);
    if nx > 0.34 && (ny - branch_y).abs() < 0.013 {
        return [78, 56, 42];
    }
    // Pine needle clusters near upper-right
    let nd = (((nx - 0.76) * 1.4).powi(2) + (ny - 0.16).powi(2)).sqrt();
    if nd < 0.13 && ((nx - 0.76).atan2(ny - 0.16) * 14.0).cos() > 0.25 {
        let shade = (1.0 - nd / 0.13) * 75.0;
        return [28 + shade as u8 / 2, 58 + shade as u8, 48 + shade as u8 / 2];
    }
    // Hanging cord
    if (0.14..=0.25).contains(&ny) && (nx - 0.50).abs() < 0.006 {
        return [140, 118, 86];
    }

    let dx = (nx - 0.50) * 1.5;
    let dy = (ny - 0.50) / 0.25;
    if dy.abs() <= 1.0 {
        let half_w = 0.245 * (1.0 - 0.36 * dy * dy).max(0.0).sqrt();
        if dx.abs() <= half_w {
            if dy.abs() > 0.89 {
                let rim = 1.0 - (dx.abs() / half_w.max(1e-4)) * 0.55;
                return [(78.0 * rim) as u8, (52.0 * rim) as u8, (36.0 * rim) as u8];
            }
            let u_x = dx / half_w.max(1e-4);
            let core = (1.0 - (u_x * u_x * 0.72 + dy * dy * 0.58)).clamp(0.0, 1.0);
            let rib = ((dy + 1.0) * 5.0 * std::f64::consts::PI).cos().abs();
            let rib_dim = if rib > 0.93 { 0.48 } else { 1.0 };
            return [
                ((165.0 + core * 90.0) * rib_dim) as u8,
                ((58.0 + core * 182.0) * rib_dim) as u8,
                ((28.0 + core * 148.0) * rib_dim) as u8,
            ];
        }
    }

    // Silk tassel below bottom rim
    if (0.75..=0.91).contains(&ny) {
        let spread = 0.012 + (ny - 0.75) * 0.11;
        if (nx - 0.50).abs() < spread {
            let strand = (((nx - 0.50) / spread) * 9.0).cos().abs();
            let fade = 1.0 - (ny - 0.75) / 0.18;
            let v = (0.45 + 0.55 * strand) * fade;
            return [(215.0 * v) as u8, (82.0 * v) as u8, (44.0 * v) as u8];
        }
    }

    let r_halo = (dx * dx + (ny - 0.50).powi(2)).sqrt();
    let halo = (1.0 - (r_halo / 0.40).min(1.0)).powi(4) * 30.0;
    let s = star_spark(nx, ny, 73);
    [
        (8.0 + halo) as u8 | s,
        (9.0 + halo * 0.55) as u8 | s,
        (15.0 + halo * 0.25) as u8 | s,
    ]
}

fn sample_nebula(nx: f64, ny: f64) -> [u8; 3] {
    // Tilted grand two-armed spiral galaxy (Andromeda-style foreshortened disc).
    let u = (nx - 0.5) * 1.45;
    let v = ny - 0.5;
    let (sin_t, cos_t) = (-0.28_f64).sin_cos();
    let dx = (u * cos_t - v * sin_t) * 0.92;
    let dy = (u * sin_t + v * cos_t) * 1.85;
    let r = (dx * dx + dy * dy).sqrt();
    let a = dy.atan2(dx);
    let arm = ((2.0 * a - r * 9.5).cos() * 0.5 + 0.5).powi(3);
    let core = (1.0 - (r / 0.34).min(1.0)).powi(2);
    let env = (1.0 - (r / 0.78).min(1.0)).powf(1.5);
    let disc = (1.0 - (r / 0.62).min(1.0)).powi(2) * 0.24;
    let lum = (core * 0.96 + arm * env * 0.84 + disc).clamp(0.0, 1.0);
    let s = star_spark(nx, ny, 19);
    let cyan_mix = (r / 0.62).clamp(0.0, 1.0);
    [
        ((8.0 + lum * (250.0 - cyan_mix * 95.0)) as u8).max(s),
        ((10.0 + lum * (192.0 + cyan_mix * 42.0)) as u8).max(s),
        ((16.0 + lum * 242.0) as u8).max(s),
    ]
}

fn sample_ringed_planet(nx: f64, ny: f64) -> [u8; 3] {
    // Tilted banded gas giant with wide dark inner ring gap, Cassini division, and ring shadow.
    let u = (nx - 0.5) * 1.5;
    let v = ny - 0.5;
    let (sin_t, cos_t) = 0.26_f64.sin_cos();
    let ru = u * cos_t - v * sin_t;
    let rv = u * sin_t + v * cos_t;

    let rp = 0.215;
    let d2 = ru * ru + rv * rv;
    let on_planet = d2 < rp * rp;

    // Foreshortened ring ellipse with a wide empty gap (0.215..0.345) so both ring handles frame dark space.
    let ring_r = (ru * ru + (rv * 3.1) * (rv * 3.1)).sqrt();
    let in_ring = (0.345..=0.645).contains(&ring_r) && !(0.490..=0.528).contains(&ring_r);
    let front_ring = in_ring && rv > 0.0;
    let back_ring = in_ring && rv <= 0.0;

    let ring_rgb = |rr: f64, shadowed: bool| -> [u8; 3] {
        let fine = 0.84 + 0.16 * ((rr - 0.345) * 85.0).sin();
        let outer_dim = if rr > 0.528 { 0.80 } else { 1.0 };
        let sh = if shadowed { 0.20 } else { 1.0 };
        let l = (fine * outer_dim * sh).clamp(0.0, 1.0);
        [
            (28.0 + l * 214.0) as u8,
            (32.0 + l * 214.0) as u8,
            (40.0 + l * 208.0) as u8,
        ]
    };

    if front_ring {
        return ring_rgb(ring_r, false);
    }
    if on_planet {
        let nz = ((rp * rp - d2).max(0.0)).sqrt() / rp;
        let nx3 = ru / rp;
        let ny3 = rv / rp;
        let mut diff = (-0.48 * nx3 - 0.42 * ny3 + 0.77 * nz).clamp(0.22, 1.0);
        // Crisp dark shadow right behind/under the front ring band
        let proj_r = (ru * ru + ((rv - 0.032) * 3.1).powi(2)).sqrt();
        if rv > 0.0 && (0.345..=0.645).contains(&proj_r) {
            diff *= 0.16;
        }
        let band = 0.80 + 0.20 * (ny3 * 16.0).cos();
        let l = (diff * band).clamp(0.0, 1.0);
        return [
            (28.0 + l * 224.0) as u8,
            (20.0 + l * 146.0) as u8,
            (18.0 + l * 88.0) as u8,
        ];
    }
    if back_ring {
        let shadowed = ru > 0.06 && rv.abs() < rp * 0.55;
        return ring_rgb(ring_r, shadowed);
    }

    let s = star_spark(nx, ny, 108);
    [7 | s, 9 | s, 15 | s]
}

fn sample_lighthouse(nx: f64, ny: f64) -> [u8; 3] {
    // Striped coastal lighthouse on a rocky headland with sweeping searchlights.
    let sea_y = 0.75;
    if ny >= sea_y {
        let depth = (ny - sea_y) / (1.0 - sea_y);
        let surf_zone = (0.10..=0.46).contains(&nx)
            && ny < sea_y + 0.052 * (1.0 - (nx - 0.28).abs() * 4.2).max(0.0);
        if surf_zone {
            let foam = 0.72 + 0.28 * (nx * 110.0 + ny * 90.0).sin();
            let v = (182.0 + foam * 70.0) as u8;
            return [v.saturating_sub(10), v, v];
        }
        let tower_ref = (1.0 - ((nx - 0.28).abs() * 7.5).min(1.0)) * (1.0 - depth * 0.65);
        let beam_ref = ((nx - 0.28).max(0.0) * 1.5).min(1.0) * (1.0 - depth * 0.75) * 0.62;
        let ripple = (depth * 95.0 + nx * 24.0).sin() * 0.5 + 0.5;
        let glint = (tower_ref * 0.90 + beam_ref * 0.75) * (0.25 + 0.75 * ripple);
        return [
            (14.0 + glint * 222.0) as u8,
            (20.0 + glint * 202.0) as u8,
            (34.0 + glint * 158.0) as u8,
        ];
    }

    // Sun-kissed rocky headland on the left
    let cliff_top = 0.62 + (nx - 0.27).powi(2) * 2.4 + 0.014 * (nx * 40.0).sin();
    if nx < 0.47 && ny >= cliff_top {
        let slope_lit = if nx >= 0.26 { 0.95 } else { 0.58 };
        let ridge = (1.0 - ((ny - cliff_top) * 5.0).min(1.0)) * 0.35;
        let crag = 0.78 + 0.22 * (nx * 65.0 + ny * 35.0).cos();
        let v = (48.0 + (slope_lit * crag + ridge) * 86.0).min(175.0) as u8;
        return [v, v.saturating_sub(4), v.saturating_add(14)];
    }

    // Lighthouse tower at nx = 0.28, ny in 0.26..=0.65
    let tx = nx - 0.28;
    if (0.26..=0.65).contains(&ny) {
        let half_w = 0.028 + (ny - 0.26) * 0.056;
        if tx.abs() <= half_w {
            let cyl = (0.48 + 0.52 * ((tx / half_w.max(1e-4)) * 0.5 + 0.5)).clamp(0.42, 1.0);
            let stripe = ((ny - 0.26) * 19.0).floor() as i32 % 2 == 0;
            return if stripe {
                [
                    (252.0 * cyl) as u8,
                    (246.0 * cyl) as u8,
                    (234.0 * cyl) as u8,
                ]
            } else {
                [(242.0 * cyl) as u8, (96.0 * cyl) as u8, (74.0 * cyl) as u8]
            };
        }
    }
    // Gallery deck, blazing lantern room & cupola dome
    if (0.25..0.265).contains(&ny) && tx.abs() <= 0.038 {
        return [165, 170, 182];
    }
    if (0.195..0.25).contains(&ny) && tx.abs() <= 0.030 {
        if ny < 0.220 {
            if tx.abs() <= 0.028 * (1.0 - (0.220 - ny) / 0.026).max(0.0) {
                return [112, 120, 138];
            }
        } else {
            return [255, 252, 224];
        }
    }

    // Sweeping searchlight beams from (0.28, 0.236)
    let bx = (nx - 0.28) * 1.5;
    let by = ny - 0.236;
    let dist = (bx * bx + by * by).sqrt();
    let mut beam = 0.0_f64;
    if bx > 0.0 {
        let center_y = 0.045 * bx;
        let half_cone = 0.020 + bx * 0.165;
        let perp = (by - center_y).abs();
        if perp < half_cone {
            let cross = (1.0 - (perp / half_cone).powi(2)).powf(1.2);
            let fade = (1.0 - (bx / 1.18).min(1.0)).powf(0.68);
            beam = beam.max(cross * fade * 0.94);
        }
    } else {
        let ax = -bx;
        let center_y = 0.025 * ax;
        let half_cone = 0.016 + ax * 0.145;
        let perp = (by - center_y).abs();
        if perp < half_cone {
            let cross = (1.0 - (perp / half_cone).powi(2)).powf(1.3);
            let fade = (1.0 - (ax / 0.52).min(1.0)).powf(0.8);
            beam = beam.max(cross * fade * 0.68);
        }
    }
    let lamp_glow = (1.0 - (dist / 0.16).min(1.0)).powi(3) * 0.90;
    let l = (beam + lamp_glow).clamp(0.0, 1.0);
    let s = if l < 0.08 { star_spark(nx, ny, 55) } else { 0 };
    [
        ((8.0 + l * 246.0) as u8).max(s),
        ((10.0 + l * 232.0) as u8).max(s),
        ((18.0 + l * 176.0) as u8).max(s),
    ]
}

fn sample_torii_moon(nx: f64, ny: f64) -> [u8; 3] {
    // Traditional floating Torii gate silhouetted against a giant harvest moon.
    let water_y = 0.75;
    let (ux, uy, mir) = if ny > water_y {
        let ripple = ((ny - water_y) * 210.0).sin() * 0.010;
        (
            (nx + ripple).clamp(0.0, 1.0),
            (water_y - (ny - water_y) * 0.92).clamp(0.0, water_y),
            true,
        )
    } else {
        (nx, ny, false)
    };

    // Torii geometry centred at ux = 0.50
    let cx = ux - 0.50;
    let kasagi_y = 0.305 - 0.42 * cx * cx;
    let on_kasagi = cx.abs() < 0.265 && (uy - kasagi_y).abs() < 0.017;
    let on_shimaki = cx.abs() < 0.240 && (uy - (kasagi_y + 0.022)).abs() < 0.010;
    let on_nuki = cx.abs() < 0.225 && (uy - 0.415).abs() < 0.014;
    let on_plaque = cx.abs() < 0.018 && (kasagi_y + 0.02..=0.415).contains(&uy);
    let pillar_x = 0.142 + (uy - 0.31).max(0.0) * 0.028;
    let on_pillar = (0.31..=water_y).contains(&uy) && (cx.abs() - pillar_x).abs() < 0.017;
    let on_foot = (water_y - 0.03..=water_y).contains(&uy) && (cx.abs() - pillar_x).abs() < 0.024;

    let mut rgb = if on_kasagi || on_shimaki || on_nuki || on_plaque || on_pillar || on_foot {
        // Dark lacquer crimson so it cuts a sharp silhouette across the bright moon,
        // while keeping rich vermilion colour in Photo mode.
        if on_kasagi && uy < kasagi_y {
            [28, 18, 22]
        } else {
            [62, 18, 20]
        }
    } else {
        // Distant misty island hills along the waterline
        let hill = water_y
            - 0.055 * (1.0 - ((ux - 0.18).abs() * 4.2).min(1.0))
            - 0.045 * (1.0 - ((ux - 0.83).abs() * 4.6).min(1.0))
            - 0.012;
        if uy > hill {
            [22, 18, 28]
        } else {
            // Giant harvest moon centred at (0.50, 0.44)
            let mdx = (ux - 0.50) * 1.5;
            let mdy = uy - 0.44;
            let mr = (mdx * mdx + mdy * mdy).sqrt();
            if mr < 0.305 {
                let edge_soft = ((0.305 - mr) / 0.015).clamp(0.0, 1.0);
                // Subtle lunar mare texture so interior has rich ASCII tonal detail
                let mare =
                    0.86 + 0.14 * (mdx * 15.0 + mdy * 9.0).sin() * (mdx * 8.0 - mdy * 13.0).cos();
                let vert = ((uy - 0.14) / 0.58).clamp(0.0, 1.0);
                let r = (254.0 * mare * edge_soft) as u8;
                let g = ((236.0 - vert * 78.0) * mare * edge_soft) as u8;
                let b = ((168.0 - vert * 96.0) * mare * edge_soft) as u8;
                [r, g, b]
            } else {
                let glow = (1.0 - ((mr - 0.305) / 0.18).clamp(0.0, 1.0)).powi(3) * 28.0;
                let s = if mr > 0.38 { star_spark(ux, uy, 88) } else { 0 };
                [
                    ((10.0 + glow * 1.2) as u8).max(s),
                    ((9.0 + glow * 0.65) as u8).max(s),
                    ((16.0 + glow * 0.45) as u8).max(s),
                ]
            }
        }
    };

    if mir {
        rgb = [
            (u16::from(rgb[0]) * 65 / 100) as u8,
            (u16::from(rgb[1]) * 62 / 100) as u8,
            (u16::from(rgb[2]) * 72 / 100) as u8,
        ];
    }
    rgb
}

fn sample_great_wave(nx: f64, ny: f64) -> [u8; 3] {
    // Curling Prussian-blue & turquoise wave with white foam crest and distant snow peak.
    // Foreground swell at bottom
    let swell_y = 0.83 + 0.045 * (nx * 7.5 - 0.8).sin();
    if ny > swell_y {
        let crest = (ny - swell_y) < 0.024;
        if crest {
            return [236, 244, 250];
        }
        let d = ((ny - swell_y) / 0.18).clamp(0.0, 1.0);
        return [
            (24.0 + (1.0 - d) * 48.0) as u8,
            (58.0 + (1.0 - d) * 76.0) as u8,
            (108.0 + (1.0 - d) * 72.0) as u8,
        ];
    }

    // Main arching wave outer curve + inner curling hollow around (0.55, 0.49)
    let outer_top = if nx < 0.56 {
        0.18 + ((0.48 - nx).max(0.0) * 1.32).powi(2) + ((nx - 0.48).max(0.0) * 1.6).powi(2)
    } else {
        1.2 // past the overhanging lip
    };
    let hdx = (nx - 0.54) * 1.42;
    let hdy = ny - 0.50;
    let hr = (hdx * hdx + hdy * hdy).sqrt();
    let in_hollow = hr < 0.235 && !(nx < 0.42 && ny > 0.54);

    // Overhanging white foam claws & spray around the curling lip
    let lip_dx = (nx - 0.56) * 1.4;
    let lip_dy = ny - 0.27;
    let lip_r = (lip_dx * lip_dx + lip_dy * lip_dy).sqrt();
    let claw_angle = lip_dy.atan2(lip_dx);
    let in_claws = lip_r < 0.165
        && (-0.4..=2.3).contains(&claw_angle)
        && ((claw_angle * 7.0 + lip_r * 22.0).sin() > 0.08 || lip_r < 0.075);

    if in_claws {
        return [246, 250, 255];
    }

    let in_main_wave =
        (ny >= outer_top && !in_hollow && nx < 0.64) || (ny >= 0.72 + 0.04 * (nx * 5.0).cos());
    if in_main_wave {
        let near_crest = (ny - outer_top).abs() < 0.048 && nx > 0.16 && nx < 0.58;
        let inner_rim = (hr - 0.235).abs() < 0.032 && ny < 0.52 && nx > 0.36;
        if near_crest || inner_rim {
            return [242, 248, 254];
        }
        // Translucent turquoise near top, deep Prussian blue in trough, pale striations
        let height_u = (1.0 - ((ny - 0.20) / 0.62).clamp(0.0, 1.0)).powf(1.2);
        let streak = ((nx * 28.0 + ny * 19.0).sin() * 0.5 + 0.5).powi(3) * 0.28;
        let u = (height_u * 0.78 + streak).clamp(0.0, 1.0);
        return [
            (22.0 + u * 86.0) as u8,
            (46.0 + u * 146.0) as u8,
            (96.0 + u * 128.0) as u8,
        ];
    }

    // Distant snow-capped volcanic cone on the right horizon
    let fuji_dx = (nx - 0.76).abs();
    let fuji_top = 0.57 + fuji_dx.powf(0.82) * 1.05;
    if fuji_dx > 0.018 && ny >= fuji_top && ny <= 0.76 {
        let snow_line = 0.635 + 0.015 * (nx * 45.0).sin();
        return if ny < snow_line {
            [240, 244, 252]
        } else {
            [56, 74, 114]
        };
    }
    if fuji_dx <= 0.018 && (0.585..=0.76).contains(&ny) {
        return if ny < 0.64 {
            [244, 248, 254]
        } else {
            [56, 74, 114]
        };
    }

    // Clean dramatic twilight sky so wave foam and snow cap pop sharply
    [10, 13, 22]
}

fn sample_jellyfish(nx: f64, ny: f64) -> [u8; 3] {
    // Bioluminescent jellyfish drifting diagonally in deep abyssal water.
    let u = (nx - 0.56) * 1.5;
    let v = ny - 0.34;
    let (sin_a, cos_a) = 0.42_f64.sin_cos();
    let ru = u * cos_a - v * sin_a;
    let rv = u * sin_a + v * cos_a;

    // Scalloped bell dome in rv: -0.22..=0.06
    let scallop = 0.055 + 0.018 * (ru * 38.0).cos();
    if rv >= -0.23 && rv <= scallop {
        let t = ((rv + 0.23) / 0.29).clamp(0.0, 1.0);
        let half_w = 0.27 * (1.0 - (1.0 - t).powf(2.1)).max(0.0).sqrt();
        if ru.abs() <= half_w {
            let ux = ru / half_w.max(1e-4);
            let rim = ux.abs().powi(3);
            let margin = ((rv - (scallop - 0.035)) / 0.035).clamp(0.0, 1.0);
            let canal = ((ux * std::f64::consts::PI * 3.5).cos().abs()).powi(8) * 0.55;
            let inner_pink = (1.0 - ux.abs()) * (1.0 - (t - 0.55).abs() * 2.2).max(0.0);
            let glow =
                (0.34 + rim * 0.62 + margin * 0.48 + canal + inner_pink * 0.45).clamp(0.0, 1.0);
            return [
                (38.0 + inner_pink * 205.0 + rim * 65.0).min(255.0) as u8,
                (65.0 + glow * 178.0 - inner_pink * 55.0).clamp(0.0, 255.0) as u8,
                (125.0 + glow * 130.0).min(255.0) as u8,
            ];
        }
    }

    // Trailing ruffled oral arms & slender outer tentacles in rv: 0.03..=0.64
    if (0.03..=0.64).contains(&rv) {
        let fade = (1.0 - (rv - 0.03) / 0.62).clamp(0.0, 1.0);
        let mut best_pink = 0.0_f64;
        for (offset, freq, phase) in [(-0.052, 16.0, 0.4), (0.0, 13.5, 2.1), (0.052, 15.0, 4.0)] {
            let center = offset + (rv * freq + phase).sin() * 0.040 * (0.4 + 0.6 * (1.0 - fade));
            let w = 0.028 * fade.sqrt().max(0.35);
            let d = (ru - center).abs();
            if d < w {
                best_pink = best_pink.max((1.0 - d / w).powf(0.7) * fade.powf(0.45));
            }
        }
        let mut best_cyan = 0.0_f64;
        for (offset, freq, phase) in [
            (-0.18, 11.0, 0.0),
            (-0.11, 13.0, 1.5),
            (0.11, 12.5, 3.1),
            (0.18, 10.5, 4.7),
        ] {
            let center = offset + (rv * freq + phase).sin() * 0.034;
            let w = 0.015;
            let d = (ru - center).abs();
            if d < w {
                best_cyan = best_cyan.max((1.0 - d / w).powf(0.75) * fade.powf(0.42));
            }
        }
        if best_pink > 0.02 || best_cyan > 0.02 {
            let r = (36.0 + best_pink * 218.0 + best_cyan * 65.0).min(255.0) as u8;
            let g = (48.0 + best_pink * 125.0 + best_cyan * 206.0).min(255.0) as u8;
            let b = (82.0 + best_pink * 172.0 + best_cyan * 173.0).min(255.0) as u8;
            return [r, g, b];
        }
    }

    let sp = star_spark(nx, ny, 204);
    [6 | (sp / 3), 9 | ((u16::from(sp) * 3 / 4) as u8), 16 | sp]
}

fn sample_aurora_pines(nx: f64, ny: f64) -> [u8; 3] {
    // Boreal spruce forest silhouetted directly in front of glowing northern lights.
    let lake_y = 0.78;
    let (ux, uy, mir) = if ny > lake_y {
        let ripple = ((ny - lake_y) * 195.0).sin() * 0.014;
        (
            (nx + ripple).clamp(0.0, 1.0),
            (lake_y - (ny - lake_y) * 1.15).clamp(0.0, lake_y),
            true,
        )
    } else {
        (nx, ny, false)
    };

    // Jagged boreal spruce silhouettes cutting into the glowing horizon curtain
    let trees: [(f64, f64, f64); 10] = [
        (0.06, 0.50, 0.072),
        (0.16, 0.40, 0.084),
        (0.27, 0.54, 0.064),
        (0.37, 0.60, 0.056),
        (0.48, 0.64, 0.052),
        (0.59, 0.58, 0.060),
        (0.70, 0.46, 0.078),
        (0.82, 0.37, 0.090),
        (0.93, 0.49, 0.074),
        (0.21, 0.62, 0.055),
    ];
    for (tx, apex, span) in trees {
        if uy >= apex && uy <= lake_y {
            let h_frac = (uy - apex) / (lake_y - apex);
            let tier = 0.68 + 0.32 * ((uy - apex) * 54.0).fract();
            let half_w = span * h_frac.powf(0.84) * tier;
            if (ux - tx).abs() <= half_w {
                return [8, 13, 15];
            }
        }
    }
    if uy > 0.755 {
        return [10, 16, 18];
    }

    // Two sweeping aurora curtains reaching down to the lake horizon behind every spruce tree
    let c1_base = 0.76 + 0.02 * (ux * 5.2 - 0.6).sin();
    let c2_base = 0.42 + 0.08 * (ux * 6.4 + 1.8).sin();
    let rays1 = 0.78 + 0.22 * (ux * 76.0 + uy * 6.0).sin();
    let rays2 = 0.70 + 0.30 * (ux * 64.0 - uy * 5.0 + 1.4).cos();

    let a1 = if uy <= lake_y {
        let up = (c1_base - uy).max(0.0);
        (-up * 3.6).exp() * rays1
    } else {
        0.0
    };
    let a2 = if uy <= c2_base + 0.03 {
        let up = (c2_base - uy).max(0.0);
        let bot = ((c2_base + 0.03 - uy) / 0.03).clamp(0.0, 1.0);
        (-up * 5.8).exp() * bot * rays2 * 0.88
    } else {
        0.0
    };

    let alt = ((c1_base - uy).max(0.0) / 0.42).clamp(0.0, 1.0);
    let intensity = (a1 * 0.95 + a2 * 0.85).clamp(0.0, 1.0);
    let s = if intensity < 0.10 && !mir {
        star_spark(ux, uy, 311)
    } else {
        0
    };
    let mut rgb = [
        ((7.0 + intensity * (52.0 + alt * 165.0)) as u8).max(s),
        ((9.0 + intensity * (250.0 - alt * 120.0)) as u8).max(s),
        ((16.0 + intensity * (150.0 + alt * 96.0)) as u8).max(s),
    ];
    if mir {
        rgb = [
            (u16::from(rgb[0]) * 72 / 100) as u8,
            (u16::from(rgb[1]) * 78 / 100) as u8,
            (u16::from(rgb[2]) * 82 / 100) as u8,
        ];
    }
    rgb
}

fn in_tri(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let cross = |o: (f64, f64), u: (f64, f64), v: (f64, f64)| {
        (u.0 - o.0) * (v.1 - o.1) - (u.1 - o.1) * (v.0 - o.0)
    };
    let d1 = cross(p, a, b);
    let d2 = cross(p, b, c);
    let d3 = cross(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn sample_origami_crane(nx: f64, ny: f64) -> [u8; 4] {
    // Geometric folded paper crane with distinct per-facet shading and alpha cutout.
    let p = ((nx - 0.50) * 1.5, ny - 0.48);

    // Painter's order from back to front so overlapping folds have crisp crease lines:
    let mut out: Option<[u8; 3]> = None;

    // 1. Far wing (back right)
    if in_tri(p, (-0.06, 0.00), (0.36, -0.38), (0.16, 0.06)) {
        out = Some([112, 128, 162]);
    }
    // 2. Tail spire (right)
    if in_tri(p, (0.10, 0.07), (0.46, -0.22), (0.24, 0.16)) {
        out = Some([154, 144, 148]);
    }
    // 3. Lower belly fold
    if in_tri(p, (-0.22, 0.13), (0.24, 0.16), (0.01, 0.29)) {
        out = Some([84, 80, 92]);
    }
    // 4. Central folded body diamond
    if in_tri(p, (-0.22, 0.13), (0.02, -0.10), (0.24, 0.16)) {
        out = Some([192, 184, 176]);
    }
    // 5. Neck spire (left)
    if in_tri(p, (-0.11, 0.09), (-0.33, -0.30), (-0.22, 0.13)) {
        out = Some([228, 212, 194]);
    }
    // 6. Folded head / beak tip
    if in_tri(p, (-0.33, -0.30), (-0.48, -0.19), (-0.29, -0.20)) {
        out = Some([244, 142, 102]);
    }
    // 7. Foreground main wing (sweeping upper-left to body crease)
    if in_tri(p, (-0.17, 0.04), (0.18, 0.06), (-0.07, -0.41)) {
        let lift = ((-0.04 - p.1) / 0.38).clamp(0.0, 1.0);
        out = Some([
            (216.0 + lift * 38.0) as u8,
            (204.0 + lift * 42.0) as u8,
            (190.0 + lift * 46.0) as u8,
        ]);
    }

    match out {
        Some([r, g, b]) => [r, g, b, 255],
        None => [0, 0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;

    fn split_gradient() -> RgbaImage {
        let (w, h) = (80, 48);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = (x * 255 / (w - 1)) as u8;
                let r = v;
                let g = (y * 255 / (h - 1)) as u8;
                let b = 255 - v;
                px.extend_from_slice(&[r, g, b, 255]);
            }
        }
        RgbaImage::new(w, h, px).unwrap()
    }

    #[test]
    fn dark_and_light_polarities_invert_character_weights() {
        let src = split_gradient();
        let mut cfg = AsciiConfig {
            columns: 40,
            color_mode: AsciiColorMode::Black,
            ..AsciiConfig::default()
        };
        let on_black = convert_standalone(&src, &cfg, Theme::Dark);
        let last = (on_black.cols * on_black.rows - 1) as usize;
        assert_eq!(on_black.chars[0], ' ');
        assert_eq!(on_black.chars[last], '@');

        cfg.color_mode = AsciiColorMode::White;
        let on_white = convert_standalone(&src, &cfg, Theme::Light);
        assert_eq!(on_white.chars[0], '@');
        assert_eq!(on_white.chars[last], ' ');
    }

    #[test]
    fn braille_and_custom_ramps_produce_expected_characters() {
        let src = split_gradient();
        let braille_cfg = AsciiConfig {
            columns: 32,
            ramp: RampPreset::Braille,
            ..AsciiConfig::default()
        };
        let braille = convert_standalone(&src, &braille_cfg, Theme::Dark);
        assert!(
            braille
                .chars
                .iter()
                .any(|&c| ('\u{2801}'..='\u{28FF}').contains(&c))
        );

        let custom_cfg = AsciiConfig {
            columns: 32,
            ramp: RampPreset::Custom,
            custom_ramp: "#. ".into(),
            ..AsciiConfig::default()
        };
        let custom = convert_standalone(&src, &custom_cfg, Theme::Dark);
        assert!(custom.chars.iter().all(|c| ['#', '.', ' '].contains(c)));
    }

    #[test]
    fn none_background_exports_transparent_rgba_png_in_light_and_dark_ink() {
        let src = split_gradient();
        let cfg = AsciiConfig {
            columns: 30,
            color_mode: AsciiColorMode::None,
            ..AsciiConfig::default()
        };
        let art_dark = convert_standalone(&src, &cfg, Theme::Dark);
        let (w, h, rgba_light_ink) = render_rgba(&art_dark, &cfg, Theme::Dark, 12, true);
        assert_eq!(
            rgba_light_ink[3], 0,
            "top-left margin pixel must be transparent"
        );
        let light_ink_px = rgba_light_ink
            .as_chunks::<4>()
            .0
            .iter()
            .find(|p| p[3] > 200)
            .expect("opaque light-ink pixel");
        assert!(
            light_ink_px[0] > 200,
            "Theme::Dark None mode should render light ink"
        );

        let art_light = convert_standalone(&src, &cfg, Theme::Light);
        let (_, _, rgba_dark_ink) = render_rgba(&art_light, &cfg, Theme::Light, 12, true);
        let dark_ink_px = rgba_dark_ink
            .as_chunks::<4>()
            .0
            .iter()
            .find(|p| p[3] > 200)
            .expect("opaque dark-ink pixel");
        assert!(
            dark_ink_px[0] < 40,
            "Theme::Light None mode should render dark ink"
        );

        let png = encode_rgba_png(w, h, &rgba_light_ink).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn custom_ramp_sanitizes_unmapped_unicode_and_weather_scale_uses_scene_grid() {
        // Emoji, control chars, and duplicate characters are stripped or deduplicated,
        // falling back cleanly if nothing renderable was typed.
        let fallback = RampPreset::Custom.heavy_to_light("🚀🎉\n\t   ");
        assert_eq!(fallback.last(), Some(&' '));
        assert!(fallback.len() >= 2);
        assert!(
            fallback
                .iter()
                .take(fallback.len() - 1)
                .all(|&c| GlyphAtlas::can_render_char(c))
        );

        let deduped = RampPreset::Custom.heavy_to_light("@@@###...🚀 ");
        assert_eq!(deduped, vec!['@', '#', '.', ' ']);

        // Extreme aspect ratio (400x1) and low column count (30 cols) do not panic
        // and keep `SceneGraph.grid` at standard monitor particle resolution.
        let wide = RgbaImage::new(400, 1, vec![220; 400 * 4]).unwrap();
        let cfg = AsciiConfig {
            columns: 30,
            weather: "snow".into(),
            ..AsciiConfig::default()
        };
        assert!(cfg.has_live_weather());
        let pkg = build_photo_package(&wide, &cfg, 1920, 1080, 99).unwrap();
        let g = pkg.graph(Theme::Dark).unwrap();
        let expected = clairlune_scene::grid_spec(1920, 1080);
        assert_eq!(
            (g.grid.cell_w, g.grid.cell_h),
            (expected.cell_w, expected.cell_h)
        );
    }

    #[test]
    fn all_inspire_samples_have_crisp_silhouettes_and_tonal_range() {
        let cfg = AsciiConfig {
            columns: 80,
            ..AsciiConfig::default()
        };
        for buddy in [false, true] {
            let mut seen_titles = std::collections::HashSet::new();
            for idx in 0..INSPIRE_COUNT {
                let (title, img) = inspire_sample(idx, buddy);
                assert!(seen_titles.insert(title), "duplicate title {title}");
                assert_eq!((img.width, img.height), (360, 240));
                let art = convert_standalone(&img, &cfg, Theme::Dark);
                let total = art.chars.len();
                let non_space = art.chars.iter().filter(|&&c| c != ' ').count();
                let unique_ink: std::collections::HashSet<char> =
                    art.chars.iter().copied().filter(|&c| c != ' ').collect();
                assert!(
                    non_space > total / 10 && non_space < total * 9 / 10,
                    "{title} (buddy={buddy}) coverage {non_space}/{total} should keep a clean silhouette"
                );
                assert!(
                    unique_ink.len() >= 5,
                    "{title} (buddy={buddy}) should use >= 5 tonal levels, got {:?}",
                    unique_ink
                );
            }
        }
    }

    #[test]
    fn photo_packages_save_load_and_animate_weather_in_palette_and_photo_modes() {
        let (_, src) = inspire_sample(0, true);
        let dir = std::env::temp_dir().join(format!("clairlune-img2ascii-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        for (idx, mode) in [
            AsciiColorMode::Palette("nord".into()),
            AsciiColorMode::Photo,
            AsciiColorMode::Black,
        ]
        .into_iter()
        .enumerate()
        {
            let cfg = AsciiConfig {
                columns: 80,
                ramp: RampPreset::Detailed,
                color_mode: mode,
                weather: "snow".into(),
                ..AsciiConfig::default()
            };
            let pkg = build_photo_package(&src, &cfg, 640, 360, 12345).unwrap();
            let sub = dir.join(format!("mode-{idx}"));
            pkg.save(&sub).unwrap();
            let loaded = ScenePackage::load(&sub).unwrap();
            let theme = loaded.themes()[0];
            let (still, pal) = loaded.still(theme, false).unwrap();
            assert_eq!((still.width, still.height), (640, 360));
            assert!(pal.len() <= 255);

            let mut inc = loaded.incremental(theme).unwrap();
            let dirty = loaded
                .advance(&mut inc, theme, false, 1500, Clock::Smooth)
                .unwrap();
            assert!(!dirty.is_empty(), "snow weather should animate frames");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
