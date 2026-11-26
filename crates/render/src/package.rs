//! Frozen scene packages: how saved wallpapers are kept.
//!
//! A package holds the *output* of the generator for one scene at one size,
//! not its inputs: for each theme the scene supports, the scene graph (layout,
//! character track, props, emitters, lighting), the palette, the glyph masks
//! and the baked background; plus the activity definition, the character
//! frames the scene can show and the props' pixel art. Playback reads only the
//! package, never the generator, the activity catalog or the built-in art, so
//! a saved scene plays the same after any of them change.
//!
//! ```text
//! scene/
//! ├── package.json          format, recipe, size, art, per-theme scene data
//! ├── background-dark.png   indexed, no animated elements
//! ├── background-light.png
//! ├── sprites.png           the character frames, stacked (RGBA)
//! └── journey-dark-0.png    a journey's moving layers (format 2), one per
//!                           theme and layer: glyph as grey, shade as alpha
//! ```
//!
//! What is *code* rather than data (particle motion, lighting bands, the
//! state machine, palette layout) is the package player for formats 1 and 2; a
//! committed golden package (`tests/golden/packages`) pins its output.

use crate::art::{OwnedPixmap, Pixmap, SceneArt};
use crate::atlas::GlyphAtlas;
use crate::bake::bake_static;
use crate::clock::Clock;
use crate::frame::{IncrementalFrame, PixelRect, hero_time_with_art, render_frame_with_art};
use crate::journey::JourneyBake;
use crate::palette::IndexedPalette;
use crate::{IndexedImage, encode_png};
use clairlune_scene::activity::{Activity, Catalog, builtin_catalog};
use clairlune_scene::{
    BuildError, BuildOptions, Rgb, SceneGraph, SceneRecipe, SpriteKind, Theme, build_with,
};
use clairlune_sprites::{Animation, Character, Frame, PackagedCharacter, builtin_character};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

/// The newest package format this build writes and reads. Older formats
/// stay readable. Format 2 adds a journey's moving layers; a package without
/// one is still written as format 1, so older versions can play it.
pub const PACKAGE_FORMAT: u32 = 2;

#[derive(Debug)]
pub enum PackageError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Png(String),
    /// Written by a newer Clairlune.
    UnsupportedFormat(u32),
    Invalid(String),
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::Io(e) => write!(f, "{e}"),
            PackageError::Json(e) => write!(f, "package.json: {e}"),
            PackageError::Png(e) => write!(f, "image: {e}"),
            PackageError::UnsupportedFormat(v) => write!(
                f,
                "scene package format {v} is newer than this version reads ({PACKAGE_FORMAT})"
            ),
            PackageError::Invalid(e) => write!(f, "scene package: {e}"),
        }
    }
}

impl std::error::Error for PackageError {}

impl From<std::io::Error> for PackageError {
    fn from(e: std::io::Error) -> Self {
        PackageError::Io(e)
    }
}

impl From<serde_json::Error> for PackageError {
    fn from(e: serde_json::Error) -> Self {
        PackageError::Json(e)
    }
}

/// The art a package carries.
#[derive(Clone, Debug)]
pub struct PackageArt {
    activity: Option<Activity>,
    character: PackagedCharacter,
    props: Vec<(SpriteKind, OwnedPixmap)>,
}

impl SceneArt for PackageArt {
    fn activity(&self) -> Option<&Activity> {
        self.activity.as_ref()
    }
    fn character(&self) -> &dyn Character {
        &self.character
    }
    fn prop(&self, kind: SpriteKind) -> Option<&dyn Pixmap> {
        self.props
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, p)| p as &dyn Pixmap)
    }
}

/// One theme's frozen scene.
#[derive(Clone, Debug)]
struct ThemeScene {
    theme: Theme,
    /// Laid out with Clawd; `graph_unattended` is the same with Buddy off.
    graph: SceneGraph,
    graph_unattended: SceneGraph,
    palette: IndexedPalette,
    atlas: GlyphAtlas,
    background: IndexedImage,
    hero_ms: u64,
    /// A journey's moving layers, baked on first use (they are large, and
    /// a live wallpaper only ever plays one theme).
    journey: OnceLock<Option<JourneyBake>>,
}

impl ThemeScene {
    fn journey(&self) -> Option<&JourneyBake> {
        self.journey
            .get_or_init(|| JourneyBake::new(&self.graph, &self.atlas))
            .as_ref()
    }
}

/// A frozen scene: plays without the generator.
#[derive(Clone, Debug)]
pub struct ScenePackage {
    pub recipe: SceneRecipe,
    pub width: u32,
    pub height: u32,
    /// The generator that made it (for information; playback ignores it).
    pub generator_version: u32,
    art: PackageArt,
    themes: Vec<ThemeScene>,
}

impl ScenePackage {
    /// Builds the scene in every theme it supports and freezes the result.
    pub fn capture(recipe: &SceneRecipe, width: u32, height: u32) -> Result<Self, BuildError> {
        Self::capture_with(
            recipe,
            width,
            height,
            builtin_catalog(),
            builtin_character(),
        )
    }

    /// [`capture`](Self::capture) with another activity catalog and character
    /// (the developer preview loads both from disk).
    pub fn capture_with(
        recipe: &SceneRecipe,
        width: u32,
        height: u32,
        catalog: &Catalog,
        character: &dyn Character,
    ) -> Result<Self, BuildError> {
        let mut themes = Vec::new();
        let mut graphs = Vec::new();
        for theme in [Theme::Dark, Theme::Light] {
            let opts = BuildOptions {
                width,
                height,
                theme,
                buddy: true,
            };
            match build_with(catalog, recipe, &opts) {
                Ok(g) => graphs.push(g),
                Err(BuildError::ThemeLocked(_)) => {}
                Err(e) => return Err(e),
            }
        }
        let first = graphs.first().ok_or(BuildError::ThemeLocked(Theme::Dark))?;
        let activity = first
            .activity
            .as_deref()
            .and_then(|id| catalog.get(id))
            .cloned();
        let art = PackageArt {
            character: capture_character(character, activity.as_ref()),
            props: capture_props(&graphs),
            activity,
        };
        for mut graph in graphs {
            let atlas = GlyphAtlas::for_graph(&graph);
            let palette = IndexedPalette::for_graph(&graph)
                .with_sprite_colors(character_colors(&art.character));
            let background = bake_static(&graph, &atlas);
            // The background replaces the cells; a loaded package has none.
            graph.cells = Vec::new();
            let hero_ms = hero_time_with_art(&graph, &art);
            let mut graph_unattended = graph.clone();
            graph_unattended.buddy = false;
            themes.push(ThemeScene {
                theme: graph.theme,
                graph,
                graph_unattended,
                palette,
                atlas,
                background,
                hero_ms,
                journey: OnceLock::new(),
            });
        }
        Ok(ScenePackage {
            recipe: recipe.clone(),
            width,
            height,
            generator_version: recipe.generator_version,
            art,
            themes,
        })
    }

    /// Themes this scene has, dark first.
    pub fn themes(&self) -> Vec<Theme> {
        self.themes.iter().map(|t| t.theme).collect()
    }

    /// Bytes a journey's baked layers take in a theme (baking them now), 0
    /// for a scene that stays put.
    pub fn journey_bytes(&self, theme: Theme) -> usize {
        self.theme(theme)
            .and_then(ThemeScene::journey)
            .map_or(0, JourneyBake::bytes)
    }

    /// Keeps only `theme` (when the scene has it): a wallpaper on screen
    /// plays one, and each theme carries a full-size background and its
    /// own layout. Saved after this, the package has only that theme.
    pub fn keep_only(&mut self, theme: Theme) {
        if self.theme(theme).is_some() {
            self.themes.retain(|t| t.theme == theme);
        }
    }

    fn theme(&self, theme: Theme) -> Option<&ThemeScene> {
        self.themes.iter().find(|t| t.theme == theme)
    }

    /// The still-frame time in a theme.
    pub fn hero_ms(&self, theme: Theme) -> Option<u64> {
        self.theme(theme).map(|t| t.hero_ms)
    }

    /// One frame, or `None` if the scene has no such theme.
    pub fn frame(
        &self,
        theme: Theme,
        buddy: bool,
        t_ms: u64,
        clock: Clock,
    ) -> Option<(IndexedImage, &IndexedPalette)> {
        let t = self.theme(theme)?;
        let graph = if buddy { &t.graph } else { &t.graph_unattended };
        let moved = t
            .journey()
            .map(|j| j.background_at(graph, &t.background, t_ms));
        let img = render_frame_with_art(
            moved.as_ref().unwrap_or(&t.background),
            graph,
            &t.atlas,
            &t.palette,
            t_ms,
            clock,
            &self.art,
        );
        Some((img, &t.palette))
    }

    /// A frame to play the scene live, starting from the bare background;
    /// move it with [`ScenePackage::advance`].
    pub fn incremental(&self, theme: Theme) -> Option<IncrementalFrame> {
        self.theme(theme)
            .map(|t| IncrementalFrame::new(&t.background))
    }

    /// Moves a live frame to `t_ms` and returns the rectangles that changed;
    /// the frame then equals [`ScenePackage::frame`] at that time.
    pub fn advance(
        &self,
        frame: &mut IncrementalFrame,
        theme: Theme,
        buddy: bool,
        t_ms: u64,
        clock: Clock,
    ) -> Option<Vec<PixelRect>> {
        let t = self.theme(theme)?;
        let graph = if buddy { &t.graph } else { &t.graph_unattended };
        if let Some(j) = t.journey() {
            return Some(frame.advance_journey(
                j,
                &t.background,
                graph,
                &t.atlas,
                &t.palette,
                t_ms,
                clock,
                &self.art,
            ));
        }
        Some(frame.advance(
            &t.background,
            graph,
            &t.atlas,
            &t.palette,
            t_ms,
            clock,
            &self.art,
        ))
    }

    /// The still (hero) frame in a theme.
    pub fn still(&self, theme: Theme, buddy: bool) -> Option<(IndexedImage, &IndexedPalette)> {
        let t = self.hero_ms(theme)?;
        self.frame(theme, buddy, t, Clock::Live)
    }

    /// The palette in a theme.
    pub fn palette(&self, theme: Theme) -> Option<&IndexedPalette> {
        self.theme(theme).map(|t| &t.palette)
    }

    /// The scene graph in a theme (layout, emitters, loop length).
    pub fn graph(&self, theme: Theme) -> Option<&SceneGraph> {
        self.theme(theme).map(|t| &t.graph)
    }

    /// Writes the package into `dir` (created if needed).
    pub fn save(&self, dir: &Path) -> Result<(), PackageError> {
        std::fs::create_dir_all(dir)?;
        let mut sheet = SpriteSheetWriter::default();
        let character = self
            .art
            .character
            .iter()
            .map(|((activity, state), anim)| AnimationFile {
                activity: activity.clone(),
                state: state.clone(),
                name: anim.name.clone(),
                frames: anim.frames.iter().map(|f| sheet.add(f)).collect(),
            })
            .collect();
        let journeys = self.themes.iter().any(|t| t.graph.journey.is_some());
        let file = PackageFile {
            format: if journeys { 2 } else { 1 },
            generator_version: self.generator_version,
            recipe: self.recipe.clone(),
            width: self.width,
            height: self.height,
            activity: self.art.activity.clone(),
            character,
            props: self
                .art
                .props
                .iter()
                .map(|(k, p)| PropFile {
                    kind: *k,
                    width: p.width,
                    height: p.height,
                    pixels: p.pixels.clone(),
                })
                .collect(),
            themes: self
                .themes
                .iter()
                .map(|t| ThemeFile {
                    theme: t.theme,
                    graph: t.graph.clone(),
                    palette: t.palette.colors().to_vec(),
                    atlas: t.atlas.clone(),
                    hero_ms: t.hero_ms,
                })
                .collect(),
        };
        for t in &self.themes {
            let mut png = Vec::new();
            encode_png(&t.background, &t.palette, &mut png)
                .map_err(|e| PackageError::Png(e.to_string()))?;
            std::fs::write(dir.join(background_name(t.theme)), png)?;
            if let Some(j) = &t.graph.journey {
                for (i, layer) in j.layers.iter().enumerate() {
                    let png = encode_layer(layer.cols, t.graph.grid.rows, &layer.cells)?;
                    std::fs::write(dir.join(layer_name(t.theme, i)), png)?;
                }
            }
        }
        std::fs::write(dir.join("sprites.png"), sheet.png()?)?;
        let json = serde_json::to_string(&file)?;
        let tmp = dir.join("package.json.part");
        std::fs::write(&tmp, json)?;
        std::fs::rename(tmp, dir.join("package.json"))?;
        Ok(())
    }

    /// Reads a package written by this or an older version.
    pub fn load(dir: &Path) -> Result<Self, PackageError> {
        let text = std::fs::read_to_string(dir.join("package.json"))?;
        let probe: FormatProbe = serde_json::from_str(&text)?;
        if probe.format > PACKAGE_FORMAT || probe.format == 0 {
            return Err(PackageError::UnsupportedFormat(probe.format));
        }
        let file: PackageFile = serde_json::from_str(&text)?;
        let sheet = SpriteSheetReader::open(&std::fs::read(dir.join("sprites.png"))?)?;
        let mut character = PackagedCharacter::default();
        for a in &file.character {
            let frames = a
                .frames
                .iter()
                .map(|f| sheet.frame(f))
                .collect::<Result<Vec<_>, _>>()?;
            character.insert(
                &a.activity,
                &a.state,
                Animation {
                    name: a.name.clone(),
                    frames,
                },
            );
        }
        let art = PackageArt {
            activity: file.activity,
            character,
            props: file
                .props
                .into_iter()
                .map(|p| {
                    let len = (p.width as usize).checked_mul(p.height as usize);
                    if len != Some(p.pixels.len()) || p.width == 0 || p.height == 0 {
                        return Err(PackageError::Invalid(format!("prop {:?} size", p.kind)));
                    }
                    Ok((
                        p.kind,
                        OwnedPixmap {
                            width: p.width,
                            height: p.height,
                            pixels: p.pixels,
                        },
                    ))
                })
                .collect::<Result<_, _>>()?,
        };
        if art.activity.as_ref().is_some_and(|a| !a.playable()) {
            return Err(PackageError::Invalid(
                "activity has a state with no way on".into(),
            ));
        }
        let mut themes = Vec::new();
        for t in file.themes {
            let (background, colors) =
                decode_indexed(&std::fs::read(dir.join(background_name(t.theme)))?)?;
            let palette = IndexedPalette::from_colors(t.palette);
            if colors != palette.rgb_bytes()
                || (background.width, background.height) != (file.width, file.height)
            {
                return Err(PackageError::Invalid(format!(
                    "{} background does not match its palette or size",
                    t.theme.as_str()
                )));
            }
            check_graph(&t.graph, &t.atlas, file.width, file.height)?;
            check_colors(&art, &palette)?;
            let mut graph = t.graph;
            if let Some(j) = &mut graph.journey {
                let rows = graph.grid.rows;
                for (i, layer) in j.layers.iter_mut().enumerate() {
                    let bytes = std::fs::read(dir.join(layer_name(t.theme, i)))?;
                    layer.cells = decode_layer(&bytes, layer.cols, rows)?;
                }
                let width = j.layers.last().map_or(0, |l| l.cols * graph.grid.cell_w);
                if j.ground.len() != width as usize || j.loop_ms == 0 {
                    return Err(PackageError::Invalid("journey ground or loop".into()));
                }
            }
            let mut graph_unattended = graph.clone();
            graph_unattended.buddy = false;
            themes.push(ThemeScene {
                theme: t.theme,
                graph,
                graph_unattended,
                palette,
                atlas: t.atlas,
                background,
                hero_ms: t.hero_ms,
                journey: OnceLock::new(),
            });
        }
        if themes.is_empty() {
            return Err(PackageError::Invalid("no themes".into()));
        }
        Ok(ScenePackage {
            recipe: file.recipe,
            width: file.width,
            height: file.height,
            generator_version: file.generator_version,
            art,
            themes,
        })
    }
}

/// A loaded theme's graph and atlas agree with each other and the image,
/// so playback can index without checks (a damaged file is an error, not a
/// crash).
fn check_graph(
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    width: u32,
    height: u32,
) -> Result<(), PackageError> {
    let bad = |what: &str| {
        Err(PackageError::Invalid(format!(
            "{} {what}",
            graph.theme.as_str()
        )))
    };
    if (graph.width, graph.height) != (width, height) {
        return bad("scene size differs from the package's");
    }
    let g = &graph.grid;
    let fits = |origin: u32, n: u32, cell: u32, room: u32| {
        cell > 0 && u64::from(origin) + u64::from(n) * u64::from(cell) <= u64::from(room)
    };
    if !fits(g.origin_x, g.cols, g.cell_w, width) || !fits(g.origin_y, g.rows, g.cell_h, height) {
        return bad("grid doesn't fit the image");
    }
    if !atlas.fits(g.cell_w, g.cell_h) {
        return bad("glyph atlas doesn't fit the grid");
    }
    if !graph.emitters.iter().all(|e| e.is_sound()) {
        return bad("particles");
    }
    Ok(())
}

/// Every colour of the packaged art is in a theme's palette.
fn check_colors(art: &PackageArt, palette: &IndexedPalette) -> Result<(), PackageError> {
    let known = |rgb: &Rgb| palette.sprite_index(*rgb).is_some();
    let props = art
        .props
        .iter()
        .flat_map(|(_, p)| p.pixels.iter().flatten());
    let character = art
        .character
        .iter()
        .flat_map(|(_, a)| a.frames.iter().flat_map(|f| f.pixels.iter().flatten()));
    if props.chain(character).all(known) {
        Ok(())
    } else {
        Err(PackageError::Invalid(
            "art colour missing from the palette".into(),
        ))
    }
}

fn background_name(theme: Theme) -> String {
    format!("background-{}.png", theme.as_str())
}

fn layer_name(theme: Theme, i: usize) -> String {
    format!("journey-{}-{i}.png", theme.as_str())
}

/// A journey layer's cells as a grey-and-alpha PNG: one pixel per cell,
/// the glyph as grey and the shade as alpha.
fn encode_layer(
    cols: u32,
    rows: u32,
    cells: &[clairlune_scene::Cell],
) -> Result<Vec<u8>, PackageError> {
    let png_err = |e: &dyn fmt::Display| PackageError::Png(e.to_string());
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, cols, rows);
    enc.set_color(png::ColorType::GrayscaleAlpha);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| png_err(&e))?;
    let data: Vec<u8> = cells.iter().flat_map(|c| [c.glyph, c.shade]).collect();
    w.write_image_data(&data).map_err(|e| png_err(&e))?;
    w.finish().map_err(|e| png_err(&e))?;
    Ok(out)
}

fn decode_layer(
    bytes: &[u8],
    cols: u32,
    rows: u32,
) -> Result<Vec<clairlune_scene::Cell>, PackageError> {
    let png_err = |e: &dyn fmt::Display| PackageError::Png(e.to_string());
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = dec.read_info().map_err(|e| png_err(&e))?;
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(|e| png_err(&e))?;
    if info.color_type != png::ColorType::GrayscaleAlpha
        || info.bit_depth != png::BitDepth::Eight
        || (info.width, info.height) != (cols, rows)
    {
        return Err(PackageError::Png(
            "journey layer: wrong kind or size".into(),
        ));
    }
    let glyphs = clairlune_scene::glyphs::GLYPHS.len() as u8;
    buf[..info.buffer_size()]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[glyph, shade]| {
            if glyph < glyphs {
                Ok(clairlune_scene::Cell { glyph, shade })
            } else {
                Err(PackageError::Invalid("journey layer: unknown glyph".into()))
            }
        })
        .collect()
}

/// Every `(activity, state)` the scene can show: all states of its activity
/// (the state machine may reach any), or the idle fallback.
fn capture_character(from: &dyn Character, activity: Option<&Activity>) -> PackagedCharacter {
    match activity {
        Some(a) => PackagedCharacter::capture(
            from,
            a.animation
                .states
                .keys()
                .map(|s| (a.id.as_str(), s.as_str())),
        ),
        None => PackagedCharacter::capture(from, [("", "idle")]),
    }
}

/// Every colour the captured frames use, in a stable order.
fn character_colors(c: &PackagedCharacter) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = Vec::new();
    for (_, anim) in c.iter() {
        for f in &anim.frames {
            for rgb in f.pixels.iter().flatten() {
                if !out.contains(rgb) {
                    out.push(*rgb);
                }
            }
        }
    }
    out
}

/// The pixel art of every prop the scene places, in either Buddy state.
fn capture_props(graphs: &[SceneGraph]) -> Vec<(SpriteKind, OwnedPixmap)> {
    let mut out: Vec<(SpriteKind, OwnedPixmap)> = Vec::new();
    for g in graphs {
        for s in g.props.iter().chain(&g.unattended) {
            if s.kind != SpriteKind::Character && !out.iter().any(|(k, _)| *k == s.kind) {
                out.push((s.kind, OwnedPixmap::copy_of(s.kind.map())));
            }
        }
    }
    out
}

// ---- On-disk format (1, and 2 with a journey's layer images) -----------

#[derive(Deserialize)]
struct FormatProbe {
    format: u32,
}

#[derive(Serialize, Deserialize)]
struct PackageFile {
    format: u32,
    generator_version: u32,
    recipe: SceneRecipe,
    width: u32,
    height: u32,
    activity: Option<Activity>,
    character: Vec<AnimationFile>,
    props: Vec<PropFile>,
    themes: Vec<ThemeFile>,
}

#[derive(Serialize, Deserialize)]
struct AnimationFile {
    activity: String,
    state: String,
    name: String,
    frames: Vec<FrameFile>,
}

/// A frame's place in `sprites.png` and its timing and anchors.
#[derive(Serialize, Deserialize)]
struct FrameFile {
    y: u32,
    width: u32,
    height: u32,
    duration_ms: u32,
    anchors: BTreeMap<String, (i32, i32)>,
}

#[derive(Serialize, Deserialize)]
struct PropFile {
    kind: SpriteKind,
    width: u32,
    height: u32,
    pixels: Vec<Option<Rgb>>,
}

#[derive(Serialize, Deserialize)]
struct ThemeFile {
    theme: Theme,
    graph: SceneGraph,
    palette: Vec<Rgb>,
    atlas: GlyphAtlas,
    hero_ms: u64,
}

/// Stacks frames top to bottom in one RGBA image; identical frames share
/// their pixels.
#[derive(Default)]
struct SpriteSheetWriter {
    width: u32,
    rows: Vec<(u32, u32, Vec<Option<Rgb>>)>,
    height: u32,
}

impl SpriteSheetWriter {
    fn add(&mut self, f: &Frame) -> FrameFile {
        let y = match self
            .rows
            .iter()
            .find(|(_, w, px)| *w == f.width && *px == f.pixels)
        {
            Some((y, _, _)) => *y,
            None => {
                let y = self.height;
                self.rows.push((y, f.width, f.pixels.clone()));
                self.height += f.height;
                self.width = self.width.max(f.width);
                y
            }
        };
        FrameFile {
            y,
            width: f.width,
            height: f.height,
            duration_ms: f.duration_ms,
            anchors: f.anchors.clone(),
        }
    }

    fn png(&self) -> Result<Vec<u8>, PackageError> {
        let (w, h) = (self.width.max(1), self.height.max(1));
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for (y0, fw, px) in &self.rows {
            for (i, p) in px.iter().enumerate() {
                let Some(c) = p else { continue };
                let (x, y) = (i as u32 % fw, y0 + i as u32 / fw);
                let o = ((y * w + x) * 4) as usize;
                rgba[o..o + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc
            .write_header()
            .map_err(|e| PackageError::Png(e.to_string()))?;
        writer
            .write_image_data(&rgba)
            .map_err(|e| PackageError::Png(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| PackageError::Png(e.to_string()))?;
        Ok(out)
    }
}

struct SpriteSheetReader {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl SpriteSheetReader {
    fn open(bytes: &[u8]) -> Result<Self, PackageError> {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND);
        let mut reader = dec
            .read_info()
            .map_err(|e| PackageError::Png(e.to_string()))?;
        let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| PackageError::Png(e.to_string()))?;
        if info.color_type != png::ColorType::Rgba {
            return Err(PackageError::Png("sprites.png must be RGBA".into()));
        }
        buf.truncate(info.buffer_size());
        Ok(SpriteSheetReader {
            width: info.width,
            height: info.height,
            rgba: buf,
        })
    }

    fn frame(&self, f: &FrameFile) -> Result<Frame, PackageError> {
        if f.width > self.width
            || f.y
                .checked_add(f.height)
                .is_none_or(|end| end > self.height)
        {
            return Err(PackageError::Invalid(
                "a frame lies outside sprites.png".into(),
            ));
        }
        let mut pixels = Vec::with_capacity((f.width * f.height) as usize);
        for y in f.y..f.y + f.height {
            for x in 0..f.width {
                let o = ((y * self.width + x) * 4) as usize;
                let p = &self.rgba[o..o + 4];
                pixels.push((p[3] != 0).then_some(Rgb {
                    r: p[0],
                    g: p[1],
                    b: p[2],
                }));
            }
        }
        Ok(Frame {
            width: f.width,
            height: f.height,
            pixels,
            duration_ms: f.duration_ms,
            anchors: f.anchors.clone(),
        })
    }
}

/// Decodes an 8-bit indexed PNG to its indices and palette bytes.
fn decode_indexed(bytes: &[u8]) -> Result<(IndexedImage, Vec<u8>), PackageError> {
    let png_err = |e: &dyn fmt::Display| PackageError::Png(e.to_string());
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = dec.read_info().map_err(|e| png_err(&e))?;
    let palette = reader
        .info()
        .palette
        .as_ref()
        .ok_or_else(|| PackageError::Png("background must be indexed".into()))?
        .to_vec();
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(|e| png_err(&e))?;
    if info.color_type != png::ColorType::Indexed || info.bit_depth != png::BitDepth::Eight {
        return Err(PackageError::Png("background must be 8-bit indexed".into()));
    }
    buf.truncate(info.buffer_size());
    Ok((
        IndexedImage {
            width: info.width,
            height: info.height,
            pixels: buf,
        },
        palette,
    ))
}
