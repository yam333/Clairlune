//! The scene recipe: a small, theme-independent JSON document that reproduces a
//! scene at any resolution, in either theme it supports, with or without Clawd.
//! The recipe, not the pixels, is what gets archived.

use crate::hash::hash_bytes;
use crate::palette::Theme;
use serde::{Deserialize, Serialize};

/// Bump whenever generator logic changes output for an existing recipe.
///
/// History: 1 = M1 canyon + placeholder focal group; 2 = activity-first composition (M2);
/// 3 = drawn character art, sized by the art's box.
pub const GENERATOR_VERSION: u32 = 4;
/// The oldest recipes this build still makes exactly as before (version 3:
/// the original particle pace).
pub const OLDEST_GENERATOR_VERSION: u32 = 3;

/// Every archetype id.
pub const ARCHETYPES: [&str; crate::archetype::ARCHETYPE_INFO.len()] = {
    let mut ids = [""; crate::archetype::ARCHETYPE_INFO.len()];
    let mut i = 0;
    while i < ids.len() {
        ids[i] = crate::archetype::ARCHETYPE_INFO[i].id;
        i += 1;
    }
    ids
};

/// How a scene can be drawn: `line` (glyph line art, the original) or
/// `dither` (brightness as glyph density, dithered between sizes).
pub const LOOKS: [&str; 2] = ["line", "dither"];
/// Ink contrast, softest first.
pub const CONTRASTS: [&str; 3] = ["soft", "normal", "high"];
/// How far back the camera stands: `close` (Clawd at the size he has always
/// been), `wide` (about half that, in a bigger landscape) or `vast` (about a
/// quarter, the land huge around him).
pub const ZOOMS: [&str; 3] = ["close", "wide", "vast"];

/// The focal group's size against `close`.
pub fn zoom_factor(zoom: &str) -> f64 {
    match zoom {
        "wide" => 0.5,
        "vast" => 0.25,
        _ => 1.0,
    }
}

fn close() -> String {
    "close".into()
}
fn is_close(v: &str) -> bool {
    v == "close"
}

fn line() -> String {
    "line".into()
}
fn is_line(v: &str) -> bool {
    v == "line"
}
fn normal() -> String {
    "normal".into()
}
fn is_normal(v: &str) -> bool {
    v == "normal"
}

/// Reduced aspect ratio, e.g. 16:9.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Aspect {
    pub w: u32,
    pub h: u32,
}

impl Aspect {
    /// Aspect of a pixel size, reduced by the greatest common divisor.
    pub fn from_size(width: u32, height: u32) -> Self {
        assert!(width > 0 && height > 0);
        let g = gcd(width, height);
        Self {
            w: width / g,
            h: height / g,
        }
    }

    pub fn ratio(self) -> f64 {
        f64::from(self.w) / f64::from(self.h)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Time of day. Light themes use the first three, dark themes the last three.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TimeOfDay {
    Morning,
    Noon,
    GoldenHour,
    Dusk,
    DeepDusk,
    Night,
}

impl TimeOfDay {
    pub const LIGHT: [TimeOfDay; 3] = [TimeOfDay::Morning, TimeOfDay::Noon, TimeOfDay::GoldenHour];
    pub const DARK: [TimeOfDay; 3] = [TimeOfDay::Dusk, TimeOfDay::DeepDusk, TimeOfDay::Night];

    pub fn theme(self) -> Theme {
        match self {
            TimeOfDay::Morning | TimeOfDay::Noon | TimeOfDay::GoldenHour => Theme::Light,
            TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night => Theme::Dark,
        }
    }
}

/// Composition variables. Fractions are stored as integer per-mille so the
/// recipe JSON is exact and stable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Composition {
    /// Platform top (the horizon line), per-mille of height from the top.
    pub horizon_pm: u32,
    /// Horizontal centre of the focal character, per-mille of width.
    pub focus_x_pm: u32,
    /// Width of the open centre between the canyon walls, per-mille.
    pub gap_pm: u32,
    /// Exponent of the canyon wall profile, times 1000.
    pub shape_pow_pm: u32,
    /// Extra platform width on each side of the focal group, per-mille of height.
    pub platform_margin_pm: u32,
    /// Number of cliff layers behind the platform.
    pub cliff_layers: u32,
    /// Main prop on the left of the character (`true`) or the right.
    pub prop_left: bool,
    /// Movement speed in hundredths of a cell per second (0 when stationary).
    pub motion_speed_cc: u32,
    /// Half the pacing range, per-mille of the short side (0 when not pacing).
    pub pace_half_pm: u32,
    /// Flying height above the ground, per-mille of the screen height (0 when
    /// not flying; then it isn't written, so other recipes stay as they were).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fly_lift_pm: u32,
    /// A journey's loop length in milliseconds (a whole number of particle
    /// loops), or 0 for a scene that stays put; then it isn't written.
    /// Recipes from 0.11.0 only: newer journeys set `journey_pace_c`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub journey_ms: u32,
    /// A journey's pace: how fast the ground underfoot goes by, in
    /// hundredths of a cell per second (cells grow with the screen, so it
    /// looks the same on every monitor). The loop follows from it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub journey_pace_c: u32,
    /// A journey's landmark: one big, far shape that stays put while the
    /// land in front scrolls (`terrain::landmark`), or empty for none; then
    /// it isn't written.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub landmark: String,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// One palette family id per theme.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PalettePick {
    pub light: String,
    pub dark: String,
}

/// Everything the seed decided. Theme-independent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SceneRecipe {
    pub generator_version: u32,
    pub seed: u64,
    pub aspect: Aspect,
    /// Hex of [`ContentContext::hash`] at compose time.
    pub content_ctx_hash: String,
    /// The activity id (M2). `None` until the activity engine exists.
    pub activity: Option<String>,
    pub archetype: String,
    /// Time of day used in the light theme. `None` makes the scene dark-only.
    pub light_tod: Option<TimeOfDay>,
    /// Time of day used in the dark theme. `None` makes the scene light-only.
    pub dark_tod: Option<TimeOfDay>,
    pub platform: String,
    pub props: Vec<String>,
    pub weather: String,
    pub composition: Composition,
    pub palette: PalettePick,
    /// One of [`LOOKS`]. Recipes from before looks have none: line. Not
    /// written when it is line, so they stay as they were.
    #[serde(default = "line", skip_serializing_if = "is_line")]
    pub look: String,
    /// One of [`CONTRASTS`]; normal when absent (and then not written).
    #[serde(default = "normal", skip_serializing_if = "is_normal")]
    pub contrast: String,
    /// One of [`ZOOMS`]; close when absent (and then not written, so older
    /// recipes stay as they were).
    #[serde(default = "close", skip_serializing_if = "is_close")]
    pub zoom: String,
}

impl SceneRecipe {
    /// The focal group's size against `close` (1, 1/2 or 1/4).
    pub fn zoom_factor(&self) -> f64 {
        zoom_factor(&self.zoom)
    }

    /// Time of day for a theme, or `None` if the scene is locked to the other theme.
    pub fn tod_for(&self, theme: Theme) -> Option<TimeOfDay> {
        match theme {
            Theme::Light => self.light_tod,
            Theme::Dark => self.dark_tod,
        }
    }

    /// True when the scene can only render in one theme.
    pub fn is_theme_locked(&self) -> bool {
        self.light_tod.is_none() || self.dark_tod.is_none()
    }

    /// Stable short id of the recipe (hash of its JSON), for logs and bug reports.
    pub fn hash_hex(&self) -> String {
        format!("{:016x}", hash_bytes(self.to_json().as_bytes()))
    }

    /// Pretty JSON. Field order is fixed by the struct, so output is stable.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("recipe serialises")
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Whether the scene travels (its land scrolls past).
    pub fn is_journey(&self) -> bool {
        self.composition.journey_ms > 0 || self.composition.journey_pace_c > 0
    }
}

/// One entry of the recent-history list used to avoid near-duplicates.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub activity: Option<String>,
    pub archetype: String,
    pub platform: String,
    pub prop: Option<String>,
}

/// Everything besides the seed that affects composition picks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ContentContext {
    pub enabled_archetypes: Vec<String>,
    pub enabled_weather: Vec<String>,
    pub enabled_activities: Vec<String>,
    /// Most recent last. The CLI and tests pass an empty history.
    pub recent: Vec<HistoryEntry>,
    /// Hash of the activity data files.
    pub data_hash: u64,
    /// Looks scenes may be drawn in (of [`LOOKS`]); none means line.
    #[serde(default)]
    pub enabled_looks: Vec<String>,
    /// One of [`CONTRASTS`] for every scene, or `vary` for a different one
    /// each time.
    #[serde(default = "normal")]
    pub contrast: String,
    /// The theme the scene will be shown in, if known: only activities that
    /// happen in it are picked (no daytime-only scene for a dark desktop).
    /// `None` (the CLI, tests) allows every activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    /// Zooms scenes may use (of [`ZOOMS`]); none means close.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_zooms: Vec<String>,
    /// Palettes scenes may be drawn in (of [`crate::palette::PALETTES`]);
    /// none means the classic one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_palettes: Vec<String>,
}

impl Default for ContentContext {
    fn default() -> Self {
        Self {
            enabled_archetypes: ARCHETYPES.iter().map(|s| s.to_string()).collect(),
            enabled_weather: crate::archetype::WEATHERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            enabled_activities: crate::activity::Catalog::builtin()
                .activities
                .iter()
                .map(|a| a.id.clone())
                .collect(),
            recent: Vec::new(),
            data_hash: crate::activity::builtin_data_hash(),
            enabled_looks: LOOKS.iter().map(|s| s.to_string()).collect(),
            contrast: normal(),
            theme: None,
            enabled_zooms: Vec::new(),
            enabled_palettes: Vec::new(),
        }
    }
}

impl ContentContext {
    /// Stable hash of the whole context, recorded in the recipe.
    pub fn hash(&self) -> u64 {
        hash_bytes(&serde_json::to_vec(self).expect("context serialises"))
    }
}
