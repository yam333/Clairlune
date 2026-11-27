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
    /// The palette it belongs to ([`PALETTES`]): one family per theme, or
    /// two of each in the classic one.
    pub set: &'static str,
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

/// The palettes a scene can be drawn in: `classic` (Clairlune's own, the
/// only one before 0.32) and the named ones, each a family per theme.
pub const PALETTES: [&str; 12] = [
    "classic",
    "nord",
    "gruvbox",
    "catppuccin",
    "solarized",
    "rose_pine",
    "tokyo_night",
    "phosphor",
    "amber",
    "everforest",
    "kanagawa",
    "dracula",
];

/// All palette families. Picks are made by index order, so only append.
pub const FAMILIES: [PaletteFamily; 26] = [
    // The prototype's dark palette: warm near-black, cream ink, orange embers.
    PaletteFamily {
        id: "ember_night",
        set: "classic",
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
        set: "classic",
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
        set: "classic",
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
        set: "classic",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xE4DACA),
        bg_center: Rgb::hex(0xF3EBDD),
        ink: Rgb::hex(0x29221C),
        accent: Rgb::hex(0xC8562A),
        ember: Rgb::hex(0xC24E20),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Nord: polar night and snow storm, with the aurora's orange for fire.
    PaletteFamily {
        id: "nord_night",
        set: "nord",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x242933),
        bg_center: Rgb::hex(0x2E3440),
        ink: Rgb::hex(0x88C0D0),
        accent: Rgb::hex(0xD08770),
        ember: Rgb::hex(0xEBCB8B),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "nord_day",
        set: "nord",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xD8DEE9),
        bg_center: Rgb::hex(0xECEFF4),
        ink: Rgb::hex(0x2E3440),
        accent: Rgb::hex(0xD08770),
        ember: Rgb::hex(0xBF616A),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Gruvbox: warm retro browns and cream.
    PaletteFamily {
        id: "gruvbox_night",
        set: "gruvbox",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x1D2021),
        bg_center: Rgb::hex(0x282828),
        ink: Rgb::hex(0xEBDBB2),
        accent: Rgb::hex(0xFE8019),
        ember: Rgb::hex(0xFABD2F),
        glow_strength: 0.20,
        tint_strength: 0.65,
    },
    PaletteFamily {
        id: "gruvbox_day",
        set: "gruvbox",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xEBDBB2),
        bg_center: Rgb::hex(0xFBF1C7),
        ink: Rgb::hex(0x3C3836),
        accent: Rgb::hex(0xD65D0E),
        ember: Rgb::hex(0xAF3A03),
        glow_strength: 0.10,
        tint_strength: 0.55,
    },
    // Catppuccin: Mocha by night, Latte by day; peach for fire.
    PaletteFamily {
        id: "catppuccin_night",
        set: "catppuccin",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x11111B),
        bg_center: Rgb::hex(0x1E1E2E),
        ink: Rgb::hex(0xB4BEFE),
        accent: Rgb::hex(0xFAB387),
        ember: Rgb::hex(0xF9E2AF),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "catppuccin_day",
        set: "catppuccin",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xDCE0E8),
        bg_center: Rgb::hex(0xEFF1F5),
        ink: Rgb::hex(0x4C4F69),
        accent: Rgb::hex(0xFE640B),
        ember: Rgb::hex(0xDF8E1D),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Solarized: its blue-green base and its orange.
    PaletteFamily {
        id: "solarized_night",
        set: "solarized",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x00212B),
        bg_center: Rgb::hex(0x002B36),
        ink: Rgb::hex(0x93A1A1),
        accent: Rgb::hex(0xCB4B16),
        ember: Rgb::hex(0xB58900),
        glow_strength: 0.20,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "solarized_day",
        set: "solarized",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xEEE8D5),
        bg_center: Rgb::hex(0xFDF6E3),
        ink: Rgb::hex(0x3B5058),
        accent: Rgb::hex(0xCB4B16),
        ember: Rgb::hex(0xB58900),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Rosé Pine, and Rosé Pine Dawn by day.
    PaletteFamily {
        id: "rose_pine_night",
        set: "rose_pine",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x16141F),
        bg_center: Rgb::hex(0x191724),
        ink: Rgb::hex(0xEBBCBA),
        accent: Rgb::hex(0xF6C177),
        ember: Rgb::hex(0xEB6F92),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "rose_pine_day",
        set: "rose_pine",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xF2E9E1),
        bg_center: Rgb::hex(0xFAF4ED),
        ink: Rgb::hex(0x575279),
        accent: Rgb::hex(0xD7827E),
        ember: Rgb::hex(0xEA9D34),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Tokyo Night, and its Day variant.
    PaletteFamily {
        id: "tokyo_night_night",
        set: "tokyo_night",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x16161E),
        bg_center: Rgb::hex(0x1A1B26),
        ink: Rgb::hex(0xC0CAF5),
        accent: Rgb::hex(0xFF9E64),
        ember: Rgb::hex(0xE0AF68),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "tokyo_night_day",
        set: "tokyo_night",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xD0D5E3),
        bg_center: Rgb::hex(0xE1E2E7),
        ink: Rgb::hex(0x3760BF),
        accent: Rgb::hex(0xB15C00),
        ember: Rgb::hex(0x8C6C3E),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Phosphor: a green screen; by day, green ink on pale green paper.
    PaletteFamily {
        id: "phosphor_night",
        set: "phosphor",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x020A04),
        bg_center: Rgb::hex(0x061409),
        ink: Rgb::hex(0x33FF66),
        accent: Rgb::hex(0xB8FF6A),
        ember: Rgb::hex(0xD4FF8A),
        glow_strength: 0.22,
        tint_strength: 0.50,
    },
    PaletteFamily {
        id: "phosphor_day",
        set: "phosphor",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xD6ECD8),
        bg_center: Rgb::hex(0xE9F6EA),
        ink: Rgb::hex(0x0E3B1A),
        accent: Rgb::hex(0x2F8F3A),
        ember: Rgb::hex(0x1F7A2C),
        glow_strength: 0.10,
        tint_strength: 0.45,
    },
    // Amber: an amber screen; by day, brown ink on cream.
    PaletteFamily {
        id: "amber_night",
        set: "amber",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x0D0700),
        bg_center: Rgb::hex(0x160C02),
        ink: Rgb::hex(0xFFB000),
        accent: Rgb::hex(0xFFD27A),
        ember: Rgb::hex(0xFFE0A0),
        glow_strength: 0.22,
        tint_strength: 0.50,
    },
    PaletteFamily {
        id: "amber_day",
        set: "amber",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xF1E1C4),
        bg_center: Rgb::hex(0xFBF0DC),
        ink: Rgb::hex(0x4A2A00),
        accent: Rgb::hex(0xC46A00),
        ember: Rgb::hex(0xA35400),
        glow_strength: 0.10,
        tint_strength: 0.45,
    },
    // Everforest: mossy woodland slate by night, warm parchment and sage by day.
    PaletteFamily {
        id: "everforest_night",
        set: "everforest",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x232A2E),
        bg_center: Rgb::hex(0x2D353B),
        ink: Rgb::hex(0xD3C6AA),
        accent: Rgb::hex(0xE69875),
        ember: Rgb::hex(0xDBBC7F),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "everforest_day",
        set: "everforest",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xEFEBD4),
        bg_center: Rgb::hex(0xFDF6E3),
        ink: Rgb::hex(0x465258),
        accent: Rgb::hex(0xF57D26),
        ember: Rgb::hex(0xDFA000),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Kanagawa: sumi ink and old paper (Wave by night, Lotus by day).
    PaletteFamily {
        id: "kanagawa_night",
        set: "kanagawa",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x16161D),
        bg_center: Rgb::hex(0x1F1F28),
        ink: Rgb::hex(0xDCD7BA),
        accent: Rgb::hex(0xFFA066),
        ember: Rgb::hex(0xE6C384),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "kanagawa_day",
        set: "kanagawa",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xE5DDB0),
        bg_center: Rgb::hex(0xF2ECBC),
        ink: Rgb::hex(0x43436C),
        accent: Rgb::hex(0xCC6D00),
        ember: Rgb::hex(0xC84053),
        glow_strength: 0.10,
        tint_strength: 0.50,
    },
    // Dracula, and Alucard by day.
    PaletteFamily {
        id: "dracula_night",
        set: "dracula",
        theme: Theme::Dark,
        bg_edge: Rgb::hex(0x1E1F29),
        bg_center: Rgb::hex(0x282A36),
        ink: Rgb::hex(0xBD93F9),
        accent: Rgb::hex(0xFFB86C),
        ember: Rgb::hex(0xFF79C6),
        glow_strength: 0.18,
        tint_strength: 0.60,
    },
    PaletteFamily {
        id: "dracula_day",
        set: "dracula",
        theme: Theme::Light,
        bg_edge: Rgb::hex(0xEDEAE0),
        bg_center: Rgb::hex(0xFFFBEB),
        ink: Rgb::hex(0x4D389E),
        accent: Rgb::hex(0xA34D14),
        ember: Rgb::hex(0xCB3A2A),
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

/// The classic palette's families for one theme, in table order.
pub fn families_for(theme: Theme) -> impl Iterator<Item = &'static PaletteFamily> {
    families_in("classic", theme)
}

/// One palette's families for one theme, in table order.
pub fn families_in(set: &str, theme: Theme) -> impl Iterator<Item = &'static PaletteFamily> {
    FAMILIES
        .iter()
        .filter(move |f| f.set == set && f.theme == theme)
}
