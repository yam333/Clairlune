//! Palette families (data). The render crate turns a family into an indexed
//! palette of at most 255 entries.

use serde::{Deserialize, Serialize};

/// Light or dark rendering of a scene.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

/// An sRGB colour.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Self {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
        }
    }

    /// Linear mix in sRGB space, `t = 0` gives `self`. Rounded, deterministic.
    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        fn ch(a: u8, b: u8, t: f64) -> u8 {
            let v = f64::from(a) + (f64::from(b) - f64::from(a)) * t;
            v.round().clamp(0.0, 255.0) as u8
        }
        Rgb {
            r: ch(self.r, other.r, t),
            g: ch(self.g, other.g, t),
            b: ch(self.b, other.b, t),
        }
    }
}

/// A hand-tuned palette for one theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaletteFamily {
    pub id: &'static str,
    pub theme: Theme,
    /// Background at the screen edges (vignette band 0).
    pub bg_edge: Rgb,
    /// Background at the screen centre (vignette band 3).
    pub bg_center: Rgb,
    /// Glyph ink at full strength.
    pub ink: Rgb,
    /// Warm light: glow bands and glyph tint near light sources.
    pub accent: Rgb,
    /// Ember particle colour.
    pub ember: Rgb,
    /// How far the hottest glow band moves from `bg_center` toward `accent`.
    pub glow_strength: f64,
    /// How far ink in the hottest glow band moves toward `accent`.
    pub tint_strength: f64,
}

/// All palette families. Picks are made by index order, so only append.
pub const FAMILIES: [PaletteFamily; 4] = [
    // The prototype's dark palette: warm near-black, cream ink, orange embers.
    PaletteFamily {
        id: "ember_night",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x151412),
        bg_center: Rgb::hex(0x211F1C), // #151412 brightened by 55 %
        ink: Rgb::hex(0xE9E5D7),
        accent: Rgb::hex(0xFF9646),
        ember: Rgb::hex(0xFF9646),
        glow_strength: 0.20,
        tint_strength: 0.70,
    },
    PaletteFamily {
        id: "slate_night",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x121416),
        bg_center: Rgb::hex(0x1C1F23),
        ink: Rgb::hex(0xDCE1E4),
        accent: Rgb::hex(0xFF9A52),
        ember: Rgb::hex(0xFFA25E),
        glow_strength: 0.17,
        tint_strength: 0.62,
    },
    // Paper and near-black ink: the daytime look.
    PaletteFamily {
        id: "paper_day",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xE2DFD4),
        bg_center: Rgb::hex(0xF2EFE6),
        ink: Rgb::hex(0x1C1A17),
        accent: Rgb::hex(0xD9652B),
        ember: Rgb::hex(0xCF5320),
        glow_strength: 0.10,
        tint_strength: 0.55,
    },
    PaletteFamily {
        id: "sand_day",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xE4DACA),
        bg_center: Rgb::hex(0xF3EBDD),
        ink: Rgb::hex(0x29221C),
        accent: Rgb::hex(0xC8562A),
        ember: Rgb::hex(0xC24E20),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
];

/// A stand-in for fields that don't travel with saved scenes.
pub(crate) fn fallback_family() -> PaletteFamily {
    FAMILIES[0]
}

/// Looks up a family by id.
pub fn family(id: &str) -> Option<&'static PaletteFamily> {
    FAMILIES.iter().find(|f| f.id == id)
}

/// Families for one theme, in table order.
pub fn families_for(theme: Theme) -> impl Iterator<Item = &'static PaletteFamily> {
    FAMILIES.iter().filter(move |f| f.theme == theme)
}
