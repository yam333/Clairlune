//! Decoding and validating an exported sprite sheet into animations.

use crate::aseprite::{Export, Slice};
use clairlune_scene::Rgb;
use std::collections::BTreeMap;
use std::fmt;

/// Palette budget for all character colours (PLAN.md section 4.5).
pub const MAX_CHARACTER_COLORS: usize = 16;
/// Every frame needs this anchor: feet baseline x body centre.
pub const ORIGIN: &str = "origin";

/// One animation frame. Pixels are row-major, `None` is transparent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Option<Rgb>>,
    pub duration_ms: u32,
    /// Anchor points in frame pixels (`origin`, `hand_l`, `item_tip`, ...).
    pub anchors: BTreeMap<String, (i32, i32)>,
}

impl Frame {
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgb> {
        self.pixels[(y * self.width + x) as usize]
    }

    /// The `origin` anchor (validated to exist).
    pub fn origin(&self) -> (i32, i32) {
        self.anchors[ORIGIN]
    }
}

/// A tagged animation (one state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub name: String,
    pub frames: Vec<Frame>,
}

impl Animation {
    pub fn duration_ms(&self) -> u64 {
        self.frames.iter().map(|f| u64::from(f.duration_ms)).sum()
    }

    /// Frame shown `elapsed_ms` after the state started. Looping animations wrap;
    /// one-shots hold their last frame.
    pub fn frame_at(&self, elapsed_ms: u64, looping: bool) -> &Frame {
        let total = self.duration_ms().max(1);
        let mut t = if looping {
            elapsed_ms % total
        } else {
            elapsed_ms.min(total - 1)
        };
        for f in &self.frames {
            let d = u64::from(f.duration_ms);
            if t < d {
                return f;
            }
            t -= d;
        }
        self.frames.last().expect("animations have frames")
    }
}

/// A decoded sheet: one animation per tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSheet {
    pub animations: BTreeMap<String, Animation>,
    /// Distinct opaque colours, in first-seen order.
    pub colors: Vec<Rgb>,
    pub source_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetError {
    Json(String),
    Png(String),
    Rotated(String),
    OutOfBounds(String),
    PartialAlpha {
        frame: usize,
        x: u32,
        y: u32,
        alpha: u8,
    },
    TooManyColors(usize),
    BadTag(String),
    MissingOrigin {
        tag: String,
        frame: usize,
    },
    NoTags,
}

impl fmt::Display for SheetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SheetError::Json(e) => write!(f, "sheet JSON: {e}"),
            SheetError::Png(e) => write!(f, "sheet PNG: {e}"),
            SheetError::Rotated(n) => write!(f, "frame `{n}` is rotated; export without rotation"),
            SheetError::OutOfBounds(n) => write!(f, "frame `{n}` lies outside the sheet image"),
            SheetError::PartialAlpha { frame, x, y, alpha } => write!(
                f,
                "frame {frame} pixel ({x}, {y}) has alpha {alpha}; pixels must be fully opaque or transparent"
            ),
            SheetError::TooManyColors(n) => {
                write!(
                    f,
                    "{n} colours; the character may use at most {MAX_CHARACTER_COLORS}"
                )
            }
            SheetError::BadTag(t) => write!(f, "tag `{t}` has an invalid frame range or direction"),
            SheetError::MissingOrigin { tag, frame } => {
                write!(f, "tag `{tag}` frame {frame} has no `{ORIGIN}` slice key")
            }
            SheetError::NoTags => write!(f, "the sheet has no tags; every state needs one"),
        }
    }
}

impl std::error::Error for SheetError {}

/// Decodes a PNG to `(width, height, rgba8)`.
fn decode_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), SheetError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| SheetError::Png(e.to_string()))?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| SheetError::Png("image too large".into()))?
    ];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| SheetError::Png(e.to_string()))?;
    let (w, h) = (info.width, info.height);
    let n = (w * h) as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..n * 4].to_vec(),
        png::ColorType::Rgb => buf[..n * 3]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..n * 2]
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Grayscale => buf[..n].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => {
            return Err(SheetError::Png("indexed PNG was not expanded".into()));
        }
    };
    Ok((w, h, rgba))
}

/// Anchor of a slice at a frame: the latest key at or before it, as the slice's
/// centre pixel (1x1 slices give their own pixel).
/// Slice bounds are in canvas coordinates; frames are exported untrimmed, so
/// they map 1:1 onto frame pixels.
fn anchor_at(slice: &Slice, frame: usize) -> Option<(i32, i32)> {
    let key = slice
        .keys
        .iter()
        .filter(|k| k.frame <= frame)
        .max_by_key(|k| k.frame)?;
    let b = key.bounds;
    Some(((b.x + b.w / 2) as i32, (b.y + b.h / 2) as i32))
}

impl SpriteSheet {
    /// Parses an Aseprite `json-array` export and its sheet image.
    pub fn parse(json: &str, png_bytes: &[u8]) -> Result<Self, SheetError> {
        let export: Export =
            serde_json::from_str(json).map_err(|e| SheetError::Json(e.to_string()))?;
        let (iw, ih, rgba) = decode_rgba(png_bytes)?;

        let mut colors: Vec<Rgb> = Vec::new();
        let mut frames = Vec::with_capacity(export.frames.len());
        for (i, fe) in export.frames.iter().enumerate() {
            if fe.rotated {
                return Err(SheetError::Rotated(fe.filename.clone()));
            }
            let r = fe.frame;
            if r.x + r.w > iw || r.y + r.h > ih {
                return Err(SheetError::OutOfBounds(fe.filename.clone()));
            }
            let mut pixels = Vec::with_capacity((r.w * r.h) as usize);
            for y in 0..r.h {
                for x in 0..r.w {
                    let o = (((r.y + y) * iw + r.x + x) * 4) as usize;
                    let a = rgba[o + 3];
                    match a {
                        0 => pixels.push(None),
                        255 => {
                            let c = Rgb {
                                r: rgba[o],
                                g: rgba[o + 1],
                                b: rgba[o + 2],
                            };
                            if !colors.contains(&c) {
                                colors.push(c);
                            }
                            pixels.push(Some(c));
                        }
                        alpha => {
                            return Err(SheetError::PartialAlpha {
                                frame: i,
                                x,
                                y,
                                alpha,
                            });
                        }
                    }
                }
            }
            let anchors = export
                .meta
                .slices
                .iter()
                .filter_map(|s| anchor_at(s, i).map(|p| (s.name.clone(), p)))
                .collect();
            frames.push(Frame {
                width: r.w,
                height: r.h,
                pixels,
                duration_ms: fe.duration,
                anchors,
            });
        }
        if colors.len() > MAX_CHARACTER_COLORS {
            return Err(SheetError::TooManyColors(colors.len()));
        }
        if export.meta.frame_tags.is_empty() {
            return Err(SheetError::NoTags);
        }

        let mut animations = BTreeMap::new();
        for tag in &export.meta.frame_tags {
            if tag.from > tag.to || tag.to >= frames.len() {
                return Err(SheetError::BadTag(tag.name.clone()));
            }
            let forward: Vec<usize> = (tag.from..=tag.to).collect();
            let order: Vec<usize> = match tag.direction.as_str() {
                "forward" => forward,
                "reverse" => forward.into_iter().rev().collect(),
                "pingpong" => {
                    let back: Vec<usize> = forward
                        .iter()
                        .rev()
                        .skip(1)
                        .take(forward.len().saturating_sub(2))
                        .copied()
                        .collect();
                    forward.into_iter().chain(back).collect()
                }
                _ => return Err(SheetError::BadTag(tag.name.clone())),
            };
            for &i in &order {
                if !frames[i].anchors.contains_key(ORIGIN) {
                    return Err(SheetError::MissingOrigin {
                        tag: tag.name.clone(),
                        frame: i,
                    });
                }
            }
            let anim = Animation {
                name: tag.name.clone(),
                frames: order.iter().map(|&i| frames[i].clone()).collect(),
            };
            animations.insert(tag.name.clone(), anim);
        }
        Ok(Self {
            animations,
            colors,
            source_sha256: export.meta.source_sha256,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a sheet PNG + JSON in memory: `frames` 4x4 frames side by side.
    pub(crate) fn fixture(frames: u32, alpha_of_first: u8, extra_colors: u32) -> (String, Vec<u8>) {
        let (fw, fh) = (4u32, 4u32);
        let w = fw * frames;
        let mut rgba = vec![0u8; (w * fh * 4) as usize];
        for f in 0..frames {
            for y in 1..fh {
                for x in 1..3 {
                    let o = ((y * w + f * fw + x) * 4) as usize;
                    rgba[o..o + 4].copy_from_slice(&[0xD9, 0x78, 0x5C, 255]);
                }
            }
        }
        rgba[3] = alpha_of_first; // pixel (0,0) of frame 0
        for i in 0..extra_colors {
            let o = ((i % w) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[i as u8, 7, 9, 255]);
        }
        let mut png_bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_bytes, w, fh);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().unwrap();
            wr.write_image_data(&rgba).unwrap();
        }
        let frame_json: Vec<String> = (0..frames)
            .map(|f| {
                format!(
                    r#"{{"filename":"t#{f}","frame":{{"x":{},"y":0,"w":4,"h":4}},"rotated":false,"trimmed":false,"duration":{}}}"#,
                    f * fw,
                    100 + f * 10
                )
            })
            .collect();
        let json = format!(
            r##"{{"frames":[{}],"meta":{{"app":"test","image":"t.png","frameTags":[{{"name":"idle","from":0,"to":{},"direction":"forward"}},{{"name":"bob","from":0,"to":{},"direction":"pingpong"}}],"slices":[{{"name":"origin","color":"#0000ffff","keys":[{{"frame":0,"bounds":{{"x":2,"y":3,"w":1,"h":1}}}}]}},{{"name":"hand_r","keys":[{{"frame":0,"bounds":{{"x":3,"y":2,"w":1,"h":1}}}},{{"frame":1,"bounds":{{"x":3,"y":1,"w":1,"h":1}}}}]}}],"source_sha256":"abc"}}}}"##,
            frame_json.join(","),
            frames - 1,
            frames - 1
        );
        (json, png_bytes)
    }

    #[test]
    fn parses_tags_durations_and_anchors() {
        let (json, png) = fixture(3, 0, 0);
        let sheet = SpriteSheet::parse(&json, &png).unwrap();
        assert_eq!(sheet.colors, vec![Rgb::hex(0xD9785C)]);
        assert_eq!(sheet.source_sha256.as_deref(), Some("abc"));
        let idle = &sheet.animations["idle"];
        assert_eq!(idle.frames.len(), 3);
        assert_eq!(idle.duration_ms(), 100 + 110 + 120);
        assert_eq!(idle.frames[0].origin(), (2, 3));
        // Slice keys apply until the next key.
        assert_eq!(idle.frames[0].anchors["hand_r"], (3, 2));
        assert_eq!(idle.frames[2].anchors["hand_r"], (3, 1));
        // Pingpong: 0 1 2 1.
        let bob = &sheet.animations["bob"];
        assert_eq!(bob.frames.len(), 4);
        assert_eq!(bob.frames[3], idle.frames[1]);
        // Frame lookup by time.
        assert_eq!(idle.frame_at(105, true), &idle.frames[1]);
        assert_eq!(idle.frame_at(330 + 5, true), &idle.frames[0]);
        assert_eq!(idle.frame_at(10_000, false), &idle.frames[2]);
    }

    #[test]
    fn rejects_partial_alpha_and_too_many_colours() {
        let (json, png) = fixture(2, 128, 0);
        assert!(matches!(
            SpriteSheet::parse(&json, &png),
            Err(SheetError::PartialAlpha { alpha: 128, .. })
        ));
        let (json, png) = fixture(8, 0, 20);
        assert!(matches!(
            SpriteSheet::parse(&json, &png),
            Err(SheetError::TooManyColors(_))
        ));
    }

    #[test]
    fn rejects_missing_origin() {
        let (json, png) = fixture(2, 0, 0);
        let json = json.replace("\"name\":\"origin\"", "\"name\":\"feet\"");
        assert!(matches!(
            SpriteSheet::parse(&json, &png),
            Err(SheetError::MissingOrigin { .. })
        ));
    }
}
