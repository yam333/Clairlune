//! The scene graph: a resolution-specific, theme-specific description of what to
//! draw. Static layers are a grid of glyph cells; animated elements are sprite
//! instances and particle emitters.

use crate::palette::{PaletteFamily, Theme};
use crate::particles::{Emitter, Particle};
use crate::placeholder::{
    CAMP_STOVE, CAMPFIRE, CLAWD_PLACEHOLDER, COFFEE_MUG, FALLEN_HAT, LANTERN, LOG_SEAT, PixelMap,
    SPELLBOOK, TELESCOPE,
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
    let cell_h = ((f64::from(height) / f64::from(TARGET_ROWS)).round() as u32).max(MIN_CELL_H);
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub glyph: u8,
    pub shade: u8,
}

/// Radial warm light around a light source.
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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

    /// Background band of one pixel.
    pub fn band_at(&self, px: u32, py: u32) -> u8 {
        self.band_in_row(&self.row_terms(py), px)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl SpriteKind {
    /// Every kind, so palettes can reserve all sprite colours up front.
    pub const ALL: [SpriteKind; 9] = [
        SpriteKind::Character,
        SpriteKind::Campfire,
        SpriteKind::LogSeat,
        SpriteKind::Telescope,
        SpriteKind::Lantern,
        SpriteKind::Spellbook,
        SpriteKind::CampStove,
        SpriteKind::CoffeeMug,
        SpriteKind::FallenHat,
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
        }
    }
}

/// How the character moves over time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// Fixed spot (left edge x).
    Stationary { x: i32 },
    /// Walks back and forth between two left-edge positions; one round trip per period.
    Pace {
        x_min: i32,
        x_max: i32,
        period_ms: u64,
    },
}

/// The character's placement as a pure function of time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        let (x, mirror) = match self.motion {
            Motion::Stationary { x } => (x, self.face_left),
            Motion::Pace {
                x_min,
                x_max,
                period_ms,
            } => {
                let span = i64::from(x_max - x_min);
                let half = (period_ms / 2).max(1) as i64;
                let ph = (t_ms % period_ms.max(1)) as i64;
                if ph < half {
                    ((i64::from(x_min) + span * ph / half) as i32, false)
                } else {
                    ((i64::from(x_max) - span * (ph - half) / half) as i32, true)
                }
            }
        };
        SpriteInstance {
            kind: SpriteKind::Character,
            x,
            y: self.y,
            scale: self.scale,
            mirror,
        }
    }

    /// Leftmost and rightmost left-edge positions over all time.
    pub fn x_range(&self) -> (i32, i32) {
        match self.motion {
            Motion::Stationary { x } => (x, x),
            Motion::Pace { x_min, x_max, .. } => (x_min, x_max),
        }
    }
}

/// A sprite placed in the image at an integer nearest-neighbour scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug)]
pub struct SceneGraph {
    pub width: u32,
    pub height: u32,
    pub theme: Theme,
    pub tod: TimeOfDay,
    pub buddy: bool,
    /// The recipe seed (drives the character's state machine).
    pub seed: u64,
    pub family: PaletteFamily,
    pub grid: GridSpec,
    /// Row-major, `grid.cols * grid.rows`.
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
            out.extend(self.character.map(|c| c.at(t_ms)));
        } else {
            out.extend_from_slice(&self.unattended);
        }
        out
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
