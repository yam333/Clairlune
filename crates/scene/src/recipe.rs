//! The scene recipe: a small, theme-independent JSON document that reproduces a
//! scene at any resolution, in either theme it supports, with or without Clawd.
//! The recipe, not the pixels, is what gets archived.

use crate::hash::hash_bytes;
use crate::palette::Theme;
use serde::{Deserialize, Serialize};

/// Bump whenever generator logic changes output for an existing recipe.
///
/// History: 1 = M1 canyon + placeholder focal group; 2 = activity-first composition (M2).
pub const GENERATOR_VERSION: u32 = 2;

/// The nine v1 archetype ids (PLAN.md section 4.2).
pub const ARCHETYPES: [&str; 9] = {
    let mut ids = [""; 9];
    let mut i = 0;
    while i < 9 {
        ids[i] = crate::archetype::ARCHETYPE_INFO[i].id;
        i += 1;
    }
    ids
};

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
}

impl SceneRecipe {
    /// Time of day for a theme, or `None` if the scene is locked to the other theme.
    pub fn tod_for(&self, theme: Theme) -> Option<TimeOfDay> {
        match theme {
            Theme::Light => self.light_tod,
            Theme::Dark => self.dark_tod,
        }
    }

    /// True when the scene can only render in one theme (PLAN.md section 1.2).
    pub fn is_theme_locked(&self) -> bool {
        self.light_tod.is_none() || self.dark_tod.is_none()
    }

    /// Pretty JSON. Field order is fixed by the struct, so output is stable.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("recipe serialises")
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
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
        }
    }
}

impl ContentContext {
    /// Stable hash of the whole context, recorded in the recipe.
    pub fn hash(&self) -> u64 {
        hash_bytes(&serde_json::to_vec(self).expect("context serialises"))
    }
}
