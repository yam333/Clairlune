//! The scene graph: a resolution-specific, theme-specific description of what to
//! draw. Static layers are a grid of glyph cells; animated elements are sprite
//! instances and particle emitters.

use crate::palette::{PaletteFamily, Theme};
use crate::particles::{Emitter, Particle};
use crate::placeholder::{
    BASKET, BOOK_CLOSED, BUCKET, CAMP_STOVE, CAMPFIRE, CHESS_TABLE, CLAWD_PLACEHOLDER, COFFEE_MUG,
    EASEL, FALLEN_HAT, GUITAR_REST, KITE_REST, LANTERN, LOG_SEAT, PICNIC_BASKET, PRACTICE_WALL,
    PixelMap, SKIS_REST, SNOWMAN, SPELLBOOK, STONE_PILE, TELESCOPE,
};
use crate::recipe::TimeOfDay;

/// Advance width of the bundled monospace font (JetBrains Mono) per em.
pub const ADVANCE_RATIO: f64 = 0.6;
/// Font pixel size relative to the cell height.
pub const FONT_TO_CELL: f64 = 0.9;
/// Target number of rows, so the look is consistent from 720p to 4K.
pub const TARGET_ROWS: u32 = 108;
pub const MIN_CELL_W: u32 = 6;
pub const MIN_CELL_H: u32 = 11;

/// The character's layout box in sprite pixels (assets/clawd/design/clawd.lua:
/// a 15 px body with 2 px arms, 13 rows from the top edge to the feet).
pub const CHARACTER_BOX: (u32, u32) = (19, 13);

/// Number of background bands (vignette + glow, quantised).
pub const BANDS: u8 = 8;
/// Width of the ordered-dither zone around each band edge, in bands.
const DITHER_WIDTH: f64 = 0.4;
/// 4x4 Bayer matrix.
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Cell grid geometry for one output size.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct GridSpec {
    pub cols: u32,
    pub rows: u32,
    pub cell_w: u32,
    pub cell_h: u32,
    /// Pixel offset of the grid, centring it in the image.
    pub origin_x: u32,
    pub origin_y: u32,
    /// Font size in pixels used to rasterize the glyph atlas.
    pub font_px: f64,
}

/// Grid for a pixel size: about [`TARGET_ROWS`] rows, never smaller than 6x11 cells.
pub fn grid_spec(width: u32, height: u32) -> GridSpec {
    grid_with_rows(width, height, f64::from(TARGET_ROWS))
}

/// Grid for a pixel size at a zoom (see `recipe::zoom_factor`): stood further
/// back, the land has finer text, as far as the smallest cell allows.
pub fn grid_for(width: u32, height: u32, zoom: f64) -> GridSpec {
    if zoom >= 1.0 {
        return grid_spec(width, height);
    }
    grid_with_rows(width, height, f64::from(TARGET_ROWS) / zoom.sqrt())
}

fn grid_with_rows(width: u32, height: u32, rows: f64) -> GridSpec {
    let cell_h = ((f64::from(height) / rows).round() as u32).max(MIN_CELL_H);
    let font_px = f64::from(cell_h) * FONT_TO_CELL;
    let cell_w = ((font_px * ADVANCE_RATIO).round() as u32).max(MIN_CELL_W);
    let cols = width / cell_w;
    let rows = height / cell_h;
    GridSpec {
        cols,
        rows,
        cell_w,
        cell_h,
        origin_x: (width - cols * cell_w) / 2,
        origin_y: (height - rows * cell_h) / 2,
        font_px,
    }
}

/// One character cell. `glyph` indexes `glyphs::GLYPHS`; `shade` is ink strength.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub glyph: u8,
    pub shade: u8,
}

/// Radial warm light around a light source.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
}

/// Vignette and glow, quantised into [`BANDS`] background bands.
///
/// Band coordinate `c` is continuous in `[0, 8)`: the vignette contributes up to
/// 4 (bands 0-3, brighter toward the centre) and the glow pulls `c` toward 8
/// (bands 4-7, warmer toward the light). Ink in bands 4-7 is tinted toward the
/// accent, so glow and glyph tint come from one lookup.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    pub vignette_cx: f64,
    pub vignette_cy: f64,
    pub vignette_rx: f64,
    pub vignette_ry: f64,
    pub glow: Option<Glow>,
}

/// Row-invariant terms of the band computation.
#[derive(Clone, Copy, Debug)]
pub struct RowTerms {
    vy2: f64,
    gy2: f64,
    bayer_row: usize,
}

impl Lighting {
    #[inline]
    pub fn row_terms(&self, py: u32) -> RowTerms {
        let y = f64::from(py) + 0.5;
        let vy = (y - self.vignette_cy) / self.vignette_ry;
        let gy2 = match self.glow {
            Some(g) => {
                let d = y - g.cy;
                d * d
            }
            None => 0.0,
        };
        RowTerms {
            vy2: vy * vy,
            gy2,
            bayer_row: (py & 3) as usize,
        }
    }

    /// Background band of pixel `(px, py)`. [`Lighting::fill_row`] must agree exactly.
    #[inline]
    pub fn band_in_row(&self, rt: &RowTerms, px: u32) -> u8 {
        let x = f64::from(px) + 0.5;
        let vx = (x - self.vignette_cx) / self.vignette_rx;
        let q = (vx * vx + rt.vy2).min(1.0);
        let v = 1.0 - q;
        let cv = 3.999 * v * v;
        let c = match self.glow {
            Some(g) => {
                let dx = x - g.cx;
                let r2 = (dx * dx + rt.gy2) / (g.radius * g.radius);
                if r2 < 1.0 {
                    let t = 1.0 - r2;
                    cv + t * t * (7.999 - cv)
                } else {
                    cv
                }
            }
            None => cv,
        };
        let b = BAYER4[rt.bayer_row][(px & 3) as usize];
        let d = ((f64::from(b) + 0.5) / 16.0 - 0.5) * DITHER_WIDTH;
        (c + d).floor().clamp(0.0, f64::from(BANDS - 1)) as u8
    }

    /// Fills one image row with band indices.
    pub fn fill_row(&self, py: u32, out: &mut [u8]) {
        let rt = self.row_terms(py);
        for (px, o) in out.iter_mut().enumerate() {
            *o = self.band_in_row(&rt, px as u32);
        }
    }
}

/// What a sprite instance shows.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteKind {
    /// The character (placeholder pixel map in M1).
    Character,
    Campfire,
    /// Unattended variant: an empty log seat by the fire.
    LogSeat,
    Telescope,
    Lantern,
    Spellbook,
    CampStove,
    CoffeeMug,
    FallenHat,
    BookClosed,
    GuitarRest,
    Bucket,
    KiteRest,
    StonePile,
    Easel,
    Snowman,
    Basket,
    PicnicBasket,
    ChessTable,
    PracticeWall,
    SkisRest,
}

impl SpriteKind {
    /// How many of [`ALL`](Self::ALL) reserve their colours before the
    /// character's in a palette. Kinds added later go after them, so the
    /// palette indices of scenes made before stay the same.
    pub const BEFORE_CHARACTER: usize = 12;

    /// Every kind, so palettes can reserve all sprite colours up front. New
    /// kinds go at the end.
    pub const ALL: [SpriteKind; 21] = [
        SpriteKind::Character,
        SpriteKind::Campfire,
        SpriteKind::LogSeat,
        SpriteKind::Telescope,
        SpriteKind::Lantern,
        SpriteKind::Spellbook,
        SpriteKind::CampStove,
        SpriteKind::CoffeeMug,
        SpriteKind::FallenHat,
        SpriteKind::BookClosed,
        SpriteKind::GuitarRest,
        SpriteKind::Bucket,
        SpriteKind::KiteRest,
        SpriteKind::StonePile,
        SpriteKind::Easel,
        SpriteKind::Snowman,
        SpriteKind::Basket,
        SpriteKind::PicnicBasket,
        SpriteKind::ChessTable,
        SpriteKind::PracticeWall,
        SpriteKind::SkisRest,
    ];

    /// Layout box in sprite pixels. The character's box is the drawn art's
    /// (body plus arms, feet included), not the placeholder's.
    pub fn box_size(self) -> (u32, u32) {
        match self {
            SpriteKind::Character => CHARACTER_BOX,
            other => (other.map().width, other.map().height),
        }
    }

    pub fn map(self) -> &'static PixelMap {
        match self {
            SpriteKind::Character => &CLAWD_PLACEHOLDER,
            SpriteKind::Campfire => &CAMPFIRE,
            SpriteKind::LogSeat => &LOG_SEAT,
            SpriteKind::Telescope => &TELESCOPE,
            SpriteKind::Lantern => &LANTERN,
            SpriteKind::Spellbook => &SPELLBOOK,
            SpriteKind::CampStove => &CAMP_STOVE,
            SpriteKind::CoffeeMug => &COFFEE_MUG,
            SpriteKind::FallenHat => &FALLEN_HAT,
            SpriteKind::BookClosed => &BOOK_CLOSED,
            SpriteKind::GuitarRest => &GUITAR_REST,
            SpriteKind::Bucket => &BUCKET,
            SpriteKind::KiteRest => &KITE_REST,
            SpriteKind::StonePile => &STONE_PILE,
            SpriteKind::Easel => &EASEL,
            SpriteKind::Snowman => &SNOWMAN,
            SpriteKind::Basket => &BASKET,
            SpriteKind::PicnicBasket => &PICNIC_BASKET,
            SpriteKind::ChessTable => &CHESS_TABLE,
            SpriteKind::PracticeWall => &PRACTICE_WALL,
            SpriteKind::SkisRest => &SKIS_REST,
        }
    }
}

/// How the character moves over time.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// Fixed spot (left edge x).
    Stationary { x: i32 },
    /// Walks back and forth between two left-edge positions; one round trip per period.
    Pace {
        x_min: i32,
        x_max: i32,
        period_ms: u64,
    },
    /// Flies back and forth like `Pace`, `lift` pixels above the ground,
    /// bobbing `bob` pixels up and down once every `bob_ms`.
    Fly {
        x_min: i32,
        x_max: i32,
        period_ms: u64,
        lift: i32,
        bob: i32,
        bob_ms: u64,
    },
}

/// A smooth up-and-down of `amp` pixels with period `period_ms`, from two
/// parabolas (integer arithmetic, so the same on every platform).
pub fn bob_offset(t_ms: u64, period_ms: u64, amp: i32) -> i32 {
    let half = (period_ms / 2).max(1) as i64;
    let p = (t_ms % (2 * half as u64)) as i64;
    let q = p % half;
    let v = 4 * q * (half - q) * i64::from(amp) / (half * half);
    (if p < half { -v } else { v }) as i32
}

/// Left edge and facing at `t_ms` while going back and forth.
fn back_and_forth(x_min: i32, x_max: i32, period_ms: u64, t_ms: u64) -> (i32, bool) {
    let span = i64::from(x_max - x_min);
    let half = (period_ms / 2).max(1) as i64;
    let ph = (t_ms % period_ms.max(1)) as i64;
    if ph < half {
        ((i64::from(x_min) + span * ph / half) as i32, false)
    } else {
        ((i64::from(x_max) - span * (ph - half) / half) as i32, true)
    }
}

/// The character's placement as a pure function of time.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterTrack {
    /// Top edge (feet on the ground line).
    pub y: i32,
    pub scale: u32,
    pub motion: Motion,
    /// Facing when stationary (toward the main prop).
    pub face_left: bool,
}

impl CharacterTrack {
    /// Sprite placement at `t_ms`. Integer arithmetic only.
    pub fn at(&self, t_ms: u64) -> SpriteInstance {
        let (x, y, mirror) = match self.motion {
            Motion::Stationary { x } => (x, self.y, self.face_left),
            Motion::Pace {
                x_min,
                x_max,
                period_ms,
            } => {
                let (x, mirror) = back_and_forth(x_min, x_max, period_ms, t_ms);
                (x, self.y, mirror)
            }
            Motion::Fly {
                x_min,
                x_max,
                period_ms,
                lift,
                bob,
                bob_ms,
            } => {
                let (x, mirror) = back_and_forth(x_min, x_max, period_ms, t_ms);
                (x, self.y - lift + bob_offset(t_ms, bob_ms, bob), mirror)
            }
        };
        SpriteInstance {
            kind: SpriteKind::Character,
            x,
            y,
            scale: self.scale,
            mirror,
        }
    }

    /// Whether the character leaves the ground (flying, or on the water):
    /// its feet aren't on the platform.
    pub fn flies(&self) -> bool {
        matches!(self.motion, Motion::Fly { .. })
    }

    /// Leftmost and rightmost left-edge positions over all time.
    pub fn x_range(&self) -> (i32, i32) {
        match self.motion {
            Motion::Stationary { x } => (x, x),
            Motion::Pace { x_min, x_max, .. } | Motion::Fly { x_min, x_max, .. } => (x_min, x_max),
        }
    }
}

/// A cell of a scrolling layer that is inside the layer's silhouette but
/// draws nothing: it hides what is behind. (`Cell::default()` in a layer is
/// outside it: what is behind shows through.)
pub const HOLE: Cell = Cell {
    glyph: crate::glyphs::EMPTY,
    shade: 1,
};

/// One layer of a journey: a strip of cells as tall as the grid whose right
/// edge continues into its left edge, moving `turns` whole strip widths per
/// loop.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ScrollLayer {
    /// Strip width in cells (the period).
    pub cols: u32,
    /// Whole strip widths it moves per loop.
    pub turns: u32,
    /// `cols * grid.rows` cells, row-major: `Cell::default()` is see-through,
    /// [`HOLE`] hides what is behind. Not stored in a package's JSON.
    #[serde(skip)]
    pub cells: Vec<Cell>,
}

/// A scene whose camera travels (docs/scrolling-scenes.md): layers slide
/// past at their own speeds over the still background (sky, far peaks) and
/// all come around again after `loop_ms`. Integer arithmetic only, so every
/// frame is the same everywhere.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Journey {
    pub loop_ms: u64,
    /// Back to front; the last is the one the character walks on.
    pub layers: Vec<ScrollLayer>,
    /// Pixel y the feet stand at, for each pixel column of the last layer's
    /// strip (the highest ground under the feet there).
    pub ground: Vec<i32>,
    /// Screen x of the character's centre.
    pub feet_x: i32,
}

impl Journey {
    /// How far layer `i` has moved at `t_ms`, in pixels within its strip.
    pub fn offset_px(&self, i: usize, cell_w: u32, t_ms: u64) -> u32 {
        let l = &self.layers[i];
        let period = u64::from(l.cols) * u64::from(cell_w);
        let t = t_ms % self.loop_ms.max(1);
        ((u64::from(l.turns) * period * t / self.loop_ms.max(1)) % period.max(1)) as u32
    }

    /// Where the feet stand at `t_ms` (pixel y), given the grid's left edge.
    pub fn ground_at(&self, cell_w: u32, origin_x: u32, t_ms: u64) -> Option<i32> {
        let last = self.layers.len().checked_sub(1)?;
        let period = self.ground.len() as i64;
        if period == 0 {
            return None;
        }
        let off = i64::from(self.offset_px(last, cell_w, t_ms));
        let x = (i64::from(self.feet_x) - i64::from(origin_x) + off).rem_euclid(period);
        Some(self.ground[x as usize])
    }
}

/// A sprite placed in the image at an integer nearest-neighbour scale.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteInstance {
    pub kind: SpriteKind,
    /// Top-left pixel.
    pub x: i32,
    pub y: i32,
    pub scale: u32,
    pub mirror: bool,
}

impl SpriteInstance {
    pub fn width(&self) -> u32 {
        self.kind.box_size().0 * self.scale
    }
    pub fn height(&self) -> u32 {
        self.kind.box_size().1 * self.scale
    }
}

/// Everything needed to render one scene at one size and theme.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SceneGraph {
    pub width: u32,
    pub height: u32,
    pub theme: Theme,
    pub tod: TimeOfDay,
    pub buddy: bool,
    /// The recipe seed (drives the character's state machine).
    pub seed: u64,
    /// Not stored in scene packages (playback uses the package's palette).
    #[serde(skip, default = "crate::palette::fallback_family")]
    pub family: PaletteFamily,
    pub grid: GridSpec,
    /// Row-major, `grid.cols * grid.rows`. Not stored in scene packages,
    /// which keep the baked background instead.
    #[serde(skip)]
    pub cells: Vec<Cell>,
    pub lighting: Lighting,
    /// The activity id, or `None` for an archetype-first fallback scene.
    pub activity: Option<String>,
    /// Activity props, drawn in both Buddy states.
    pub props: Vec<SpriteInstance>,
    /// Unattended variant, drawn only with Buddy off.
    pub unattended: Vec<SpriteInstance>,
    /// The character's track (always laid out; drawn only with Buddy on).
    pub character: Option<CharacterTrack>,
    pub emitters: Vec<Emitter>,
    /// Pixel y of the platform's top surface (where feet stand).
    pub ground_y: i32,
    /// Hero time in milliseconds: the frame used for stills and thumbnails.
    pub t_hero_ms: u64,
    /// Seamless loop length for periodic elements.
    pub loop_ms: u64,
    /// How fast particles and effects move (`particles::CALM_PACE`); 0 in
    /// scenes saved before the calm pace, which keep their motion.
    #[serde(default)]
    pub pace: u8,
    /// The generator version that built it (0 in older saved scenes); the
    /// palette follows it, so older scenes keep their colours.
    #[serde(default)]
    pub generator: u32,
    /// A point on open water near the focal group, in pixels (the shore:
    /// just past the pier's end), for effects that reach the water.
    #[serde(default)]
    pub water: Option<(i32, i32)>,
    /// A travelling scene: layers that scroll past, ground under the feet.
    #[serde(default)]
    pub journey: Option<Journey>,
    /// The focal group's size against the closest zoom (1, 1/2, 1/4): what
    /// reaches out from it (a kite's string, a stone's skips) scales too.
    #[serde(default = "full_zoom")]
    pub zoom: f64,
}

fn full_zoom() -> f64 {
    1.0
}

impl SceneGraph {
    #[inline]
    pub fn cell(&self, col: u32, row: u32) -> Cell {
        self.cells[(row * self.grid.cols + col) as usize]
    }

    /// Sprites to draw at `t`, back to front: props, then the character (Buddy
    /// on) or the unattended variant (Buddy off).
    pub fn sprites_at(&self, t_ms: u64) -> Vec<SpriteInstance> {
        let mut out = self.props.clone();
        if self.buddy {
            out.extend(self.character.map(|c| self.on_ground(c.at(t_ms), t_ms)));
        } else {
            out.extend_from_slice(&self.unattended);
        }
        out
    }

    /// The character placed for `t_ms`: on a journey its feet follow the
    /// ground going by under them; otherwise unchanged.
    pub fn on_ground(&self, mut s: SpriteInstance, t_ms: u64) -> SpriteInstance {
        if let Some(j) = &self.journey
            && s.kind == SpriteKind::Character
            && let Some(y) = j.ground_at(self.grid.cell_w, self.grid.origin_x, t_ms)
        {
            s.y += y - self.ground_y;
        }
        s
    }

    /// Particles at time `t` that fall inside the grid, at their own pixel
    /// positions (`particle.x`, `particle.y` are their centres).
    pub fn particles_in_grid(&self, t_ms: u64) -> Vec<Particle> {
        let g = &self.grid;
        let (x0, y0) = (f64::from(g.origin_x), f64::from(g.origin_y));
        let x1 = x0 + f64::from(g.cols * g.cell_w);
        let y1 = y0 + f64::from(g.rows * g.cell_h);
        self.emitters
            .iter()
            .flat_map(|e| e.sample(t_ms))
            .filter(|p| p.x >= x0 && p.y >= y0 && p.x < x1 && p.y < y1)
            .collect()
    }

    /// Particles at time `t`, snapped to cells, as `(col, row, particle)`.
    pub fn particles_at(&self, t_ms: u64) -> Vec<(u32, u32, Particle)> {
        let g = &self.grid;
        let mut out = Vec::new();
        for e in &self.emitters {
            for p in e.sample(t_ms) {
                let cx = (p.x - f64::from(g.origin_x)) / f64::from(g.cell_w);
                let cy = (p.y - f64::from(g.origin_y)) / f64::from(g.cell_h);
                if cx < 0.0 || cy < 0.0 {
                    continue;
                }
                let (col, row) = (cx.floor() as u32, cy.floor() as u32);
                if col < g.cols && row < g.rows {
                    out.push((col, row, p));
                }
            }
        }
        out
    }
}
