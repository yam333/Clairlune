//! Design tokens (docs/design.md section 7): every colour, size, radius and
//! duration the UI may use. Toolkit-independent constants; UI code takes its
//! values from here and nowhere else.
//!
//! The colours come from the wallpaper palettes (paper and warm black) with one
//! accent, the ember orange of the wallpapers' campfires. Tests check every
//! text and control pair against WCAG 2.2 AA contrast.

// Used by the UI windows from milestone 14d on; the tests use them now.
#![allow(dead_code)]

/// An sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Rgb {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f64 {
        let lin = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }

    /// WCAG contrast ratio, 1 to 21.
    pub fn contrast(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

/// One theme's colours.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    /// The page.
    pub bg: Rgb,
    /// The navigation sidebar, one step dimmer than the page.
    pub bg_sidebar: Rgb,
    /// Rows, inputs and the one elevated surface.
    pub bg_raised: Rgb,
    /// 1 px separators.
    pub hairline: Rgb,
    /// Control outlines (3:1 against every surface).
    pub border_control: Rgb,
    pub text: Rgb,
    pub text_secondary: Rgb,
    /// Disabled controls (exempt from contrast rules, but still legible).
    pub text_disabled: Rgb,
    /// The single accent: primary action, selection, focus, "now".
    pub accent: Rgb,
    /// Text and icons on the accent.
    pub on_accent: Rgb,
    pub danger: Rgb,
}

pub const LIGHT: Colors = Colors {
    bg: Rgb::hex(0xF2EFE6),
    bg_sidebar: Rgb::hex(0xE9E5DA),
    bg_raised: Rgb::hex(0xF8F6F0),
    hairline: Rgb::hex(0xD3CEC1),
    border_control: Rgb::hex(0x847E72),
    text: Rgb::hex(0x1C1A17),
    text_secondary: Rgb::hex(0x5B564D),
    text_disabled: Rgb::hex(0x9C968A),
    accent: Rgb::hex(0xB4481C),
    on_accent: Rgb::hex(0xFFFFFF),
    danger: Rgb::hex(0xA3341F),
};

pub const DARK: Colors = Colors {
    bg: Rgb::hex(0x1A1917),
    bg_sidebar: Rgb::hex(0x151412),
    bg_raised: Rgb::hex(0x211F1C),
    hairline: Rgb::hex(0x35322D),
    border_control: Rgb::hex(0x77716A),
    text: Rgb::hex(0xE9E5D7),
    text_secondary: Rgb::hex(0xA8A296),
    text_disabled: Rgb::hex(0x5F5A52),
    accent: Rgb::hex(0xFF9646),
    on_accent: Rgb::hex(0x1A1917),
    danger: Rgb::hex(0xFF8A75),
};

/// Spacing scale in px (4 px base, 8 px rhythm). Use the named steps.
pub mod space {
    pub const S0: f32 = 0.0;
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S5: f32 = 24.0;
    pub const S6: f32 = 32.0;
    pub const S7: f32 = 48.0;
    pub const S8: f32 = 64.0;
}

/// Corner radii in px: nothing rounder than a panel.
pub mod radius {
    pub const CONTROL: f32 = 6.0;
    pub const PANEL: f32 = 10.0;
}

/// A type size: font size and line height in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeSize {
    pub size: f32,
    pub line: f32,
}

/// The five sizes (no others on any screen).
pub mod text {
    use super::TypeSize;
    /// Captions, mono labels.
    pub const SMALL: TypeSize = TypeSize {
        size: 12.0,
        line: 16.0,
    };
    /// Body, row labels, descriptions.
    pub const BODY: TypeSize = TypeSize {
        size: 14.0,
        line: 20.0,
    };
    /// Section headings.
    pub const LARGE: TypeSize = TypeSize {
        size: 16.0,
        line: 22.0,
    };
    /// Page titles.
    pub const TITLE: TypeSize = TypeSize {
        size: 20.0,
        line: 26.0,
    };
    /// The one display line (first run, empty states).
    pub const DISPLAY: TypeSize = TypeSize {
        size: 26.0,
        line: 32.0,
    };
    pub const WEIGHT_REGULAR: u16 = 400;
    /// Selected sidebar item only.
    pub const WEIGHT_MEDIUM: u16 = 500;
    pub const WEIGHT_SEMIBOLD: u16 = 600;
}

/// Sizes of things.
pub mod size {
    /// Minimum control height (WCAG 2.5.8 asks for 24).
    pub const CONTROL: f32 = 28.0;
    const _: () = assert!(CONTROL >= 24.0, "WCAG 2.5.8 target size");
    pub const BUTTON_PRIMARY: f32 = 32.0;
    pub const SIDEBAR: f32 = 200.0;
    /// Settings page content.
    pub const PAGE_MAX: f32 = 640.0;
    pub const ICON: f32 = 16.0;
    pub const ICON_STROKE: f32 = 1.5;
    pub const HAIRLINE: f32 = 1.0;
    pub const FOCUS_RING: f32 = 2.0;
    pub const FOCUS_OFFSET: f32 = 2.0;
    pub const THUMB_WIDTH: f32 = 160.0;
}

/// Motion durations in ms; ease-out only. Zero when the system asks for
/// reduced motion.
pub mod motion {
    pub const FAST: u32 = 120;
    pub const BASE: u32 = 160;
    pub const SLOW: u32 = 200;
    /// cubic-bezier(0.2, 0, 0, 1)
    pub const EASE_OUT: [f32; 4] = [0.2, 0.0, 0.0, 1.0];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, c: &Colors) {
        let surfaces = [
            ("bg", c.bg),
            ("sidebar", c.bg_sidebar),
            ("raised", c.bg_raised),
        ];
        for (s, bg) in surfaces {
            for (f, fg) in [("text", c.text), ("text_secondary", c.text_secondary)] {
                let r = fg.contrast(bg);
                assert!(r >= 4.5, "{name}: {f} on {s} is {r:.2}:1, needs 4.5");
            }
            // Control outlines, the focus ring and selection bars: non-text 3:1.
            for (f, fg) in [("border_control", c.border_control), ("accent", c.accent)] {
                let r = fg.contrast(bg);
                assert!(r >= 3.0, "{name}: {f} on {s} is {r:.2}:1, needs 3");
            }
        }
        // Accent and danger as text appear on the page and on rows only.
        for (s, bg) in [("bg", c.bg), ("raised", c.bg_raised)] {
            for (f, fg) in [("accent", c.accent), ("danger", c.danger)] {
                let r = fg.contrast(bg);
                assert!(r >= 4.5, "{name}: {f} text on {s} is {r:.2}:1, needs 4.5");
            }
        }
        let r = c.on_accent.contrast(c.accent);
        assert!(r >= 4.5, "{name}: on_accent is {r:.2}:1, needs 4.5");
        // Disabled text is exempt, but must still read as text and differ
        // from enabled secondary text.
        assert!(c.text_disabled.contrast(c.bg) >= 2.0);
        assert!(c.text_disabled.contrast(c.text_secondary) >= 1.5);
        // The hairline is quiet: visible but well below control borders.
        assert!(c.hairline.contrast(c.bg) < c.border_control.contrast(c.bg));
    }

    #[test]
    fn colours_meet_wcag_aa() {
        check("light", &LIGHT);
        check("dark", &DARK);
    }

    #[test]
    fn known_contrast_values() {
        assert!((Rgb::hex(0x000000).contrast(Rgb::hex(0xFFFFFF)) - 21.0).abs() < 1e-9);
        assert!((Rgb::hex(0x777777).contrast(Rgb::hex(0xFFFFFF)) - 4.48).abs() < 0.01);
    }

    #[test]
    fn scales_are_on_the_grid() {
        for s in [
            space::S1,
            space::S2,
            space::S3,
            space::S4,
            space::S5,
            space::S6,
            space::S7,
            space::S8,
        ] {
            assert_eq!(s % 4.0, 0.0);
        }
        for t in [
            text::SMALL,
            text::BODY,
            text::LARGE,
            text::TITLE,
            text::DISPLAY,
        ] {
            assert_eq!(t.line % 2.0, 0.0);
            assert!(t.line / t.size >= 1.2 && t.line / t.size <= 1.45, "{t:?}");
        }
    }
}
