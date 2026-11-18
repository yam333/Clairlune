//! The subset of Aseprite's `json-array` export format that we read. Unknown
//! fields are ignored, since Aseprite adds metadata between versions.

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Export {
    pub frames: Vec<FrameEntry>,
    pub meta: Meta,
}

#[derive(Deserialize, Clone, Debug)]
pub struct FrameEntry {
    #[serde(default)]
    pub filename: String,
    pub frame: Rect,
    /// Frame duration in milliseconds.
    pub duration: u32,
    #[serde(default)]
    pub rotated: bool,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Meta {
    #[serde(default)]
    pub image: String,
    #[serde(rename = "frameTags", default)]
    pub frame_tags: Vec<Tag>,
    #[serde(default)]
    pub slices: Vec<Slice>,
    /// Written by `scripts/export_sprites.py`.
    #[serde(default)]
    pub source_sha256: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    #[serde(default = "forward")]
    pub direction: String,
}

fn forward() -> String {
    "forward".to_string()
}

#[derive(Deserialize, Clone, Debug)]
pub struct Slice {
    pub name: String,
    pub keys: Vec<SliceKey>,
}

/// A slice key applies from its frame until the next key.
#[derive(Deserialize, Clone, Debug)]
pub struct SliceKey {
    pub frame: usize,
    pub bounds: Rect,
}
