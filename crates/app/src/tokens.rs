//! Design tokens (docs/design.md section 7): every colour, size, radius and
//! duration the UI may use. Toolkit-independent constants; UI code takes its
//! values from here and nowhere else.
//!
//! Moonglass: the windows are glass over the scene the app just drew, night
//! ink or day paper, with one accent, ember (Clawd's colour, the campfires'
//! glow). Tests check every text and control pair against WCAG 2.2 AA
//! contrast, on the glass's own colour (what the scene shows through is
//! dimmed to well within it).

/// An sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Rgb {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// WCAG relative luminance (the contrast tests).
    #[cfg(test)]
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
    #[cfg(test)]
    pub fn contrast(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

/// One theme's colours.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    /// The page: the content pane's glass as it reads.
    pub bg: Rgb,
    /// The sidebar's glass, one step dimmer than the page.
    pub bg_sidebar: Rgb,
    /// Cards, inputs and the one elevated surface.
    pub bg_raised: Rgb,
    /// 1 px separators.
    pub hairline: Rgb,
    /// Control outlines (3:1 against every surface).
    pub border_control: Rgb,
    pub text: Rgb,
    pub text_secondary: Rgb,
    /// Disabled controls (exempt from contrast rules, but still legible).
    pub text_disabled: Rgb,
    /// The accent as a fill: the primary button, a chosen segment's mark.
    pub accent: Rgb,
    /// Its deeper end: switches when on (3:1 against every surface) and
    /// the primary button's lower edge.
    pub accent_deep: Rgb,
    /// Text and icons on the accent.
    pub on_accent: Rgb,
    /// The accent as text, focus rings and marks on the glass (4.5:1).
    pub accent_text: Rgb,
    pub danger: Rgb,
    pub glass: Glass,
}

/// How the glass is laid over the scene.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
    /// The content pane: `pane` at this opacity over the scene, blurred.
    pub pane: Rgb,
    pub pane_alpha: f32,
    /// The sidebar's tint over the flutes, from the top to the bottom.
    pub tint: Rgb,
    pub tint_top: f32,
    pub tint_bottom: f32,
    /// Each flute's lit edge (white at this opacity) and shaded edge
    /// (`shade` at this opacity).
    pub flute_light: f32,
    pub flute_shade: f32,
    pub shade: Rgb,
    /// Cards on the glass: `card` at this opacity; the chosen sidebar item
    /// at `chosen`.
    pub card: Rgb,
    pub card_alpha: f32,
    pub chosen: f32,
    /// The small pane of smoke under each sidebar section (`pane` at this
    /// opacity), so its name reads clearly over the flutes.
    pub nav: f32,
    /// The grain over the glass.
    pub grain: f32,
}

/// The flutes: each this many px wide, showing the scene this much wider.
pub mod flute {
    pub const WIDTH: f32 = 12.0;
    pub const MAGNIFY: f32 = 1.35;
}

pub const LIGHT: Colors = Colors {
    bg: Rgb::hex(0xF6F3EC),
    bg_sidebar: Rgb::hex(0xEFEBE2),
    bg_raised: Rgb::hex(0xFCFBF8),
    hairline: Rgb::hex(0xDED8CC),
    border_control: Rgb::hex(0x847E72),
    text: Rgb::hex(0x1C1E24),
    text_secondary: Rgb::hex(0x5D5A54),
    text_disabled: Rgb::hex(0xA29C90),
    accent: Rgb::hex(0xE8875E),
    accent_deep: Rgb::hex(0xCF6640),
    on_accent: Rgb::hex(0x1A1210),
    accent_text: Rgb::hex(0xA64D29),
    danger: Rgb::hex(0xA5321F),
    glass: Glass {
        pane: Rgb::hex(0xFAF8F3),
        pane_alpha: 0.88,
        tint: Rgb::hex(0xF6F2EA),
        tint_top: 0.62,
        tint_bottom: 0.88,
        flute_light: 0.30,
        flute_shade: 0.10,
        shade: Rgb::hex(0x463A2C),
        card: Rgb::hex(0xFFFFFF),
        card_alpha: 0.55,
        chosen: 0.75,
        nav: 0.5,
        grain: 0.035,
    },
};

pub const DARK: Colors = Colors {
    bg: Rgb::hex(0x121419),
    bg_sidebar: Rgb::hex(0x0F1116),
    bg_raised: Rgb::hex(0x1B1D23),
    hairline: Rgb::hex(0x2A2C31),
    border_control: Rgb::hex(0x7A766E),
    text: Rgb::hex(0xECE6D8),
    text_secondary: Rgb::hex(0xA9A49A),
    text_disabled: Rgb::hex(0x5F5B54),
    accent: Rgb::hex(0xF0A27B),
    accent_deep: Rgb::hex(0xDD7D55),
    on_accent: Rgb::hex(0x1A1210),
    accent_text: Rgb::hex(0xF0A27B),
    danger: Rgb::hex(0xFF8A75),
    glass: Glass {
        pane: Rgb::hex(0x0D0F15),
        pane_alpha: 0.86,
        tint: Rgb::hex(0x0E1118),
        tint_top: 0.50,
        tint_bottom: 0.82,
        flute_light: 0.08,
        flute_shade: 0.20,
        shade: Rgb::hex(0x000000),
        card: Rgb::hex(0xECE6D8),
        card_alpha: 0.06,
        chosen: 0.12,
        nav: 0.42,
        grain: 0.05,
    },
};

/// Spacing scale in px (4 px base, 8 px rhythm). Use the named steps.
pub mod space {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S5: f32 = 24.0;
    pub const S6: f32 = 32.0;
    pub const S7: f32 = 48.0;
}

/// Corner radii in px: soft, like glass edges.
pub mod radius {
    pub const CONTROL: f32 = 10.0;
    pub const PANEL: f32 = 14.0;
}

/// A type size: font size and line height in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeSize {
    pub size: f32,
    pub line: f32,
}

/// The four sizes (no others on any screen).
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
    /// Window and dialog titles and the wordmark, in the serif.
    pub const TITLE: TypeSize = TypeSize {
        size: 28.0,
        line: 36.0,
    };
    /// Page titles, in the serif.
    pub const DISPLAY: TypeSize = TypeSize {
        size: 40.0,
        line: 48.0,
    };
}

/// Sizes of things.
pub mod size {
    /// Minimum control height (WCAG 2.5.8 asks for 24).
    pub const CONTROL: f32 = 32.0;
    const _: () = assert!(CONTROL >= 24.0, "WCAG 2.5.8 target size");
    pub const BUTTON_PRIMARY: f32 = 36.0;
    pub const SIDEBAR: f32 = 240.0;
    /// Settings page content.
    pub const PAGE_MAX: f32 = 640.0;
    pub const HAIRLINE: f32 = 1.0;
    pub const FOCUS_RING: f32 = 2.0;
    pub const FOCUS_OFFSET: f32 = 2.0;
}

/// Motion durations in ms; ease-out only. Zero when the system asks for
/// reduced motion.
pub mod motion {
    pub const BASE: u32 = 160;
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
            for (f, fg) in [
                ("text", c.text),
                ("text_secondary", c.text_secondary),
                ("accent_text", c.accent_text),
            ] {
                let r = fg.contrast(bg);
                assert!(r >= 4.5, "{name}: {f} on {s} is {r:.2}:1, needs 4.5");
            }
            // Control outlines, focus rings and switches when on: non-text
            // 3:1.
            for (f, fg) in [
                ("border_control", c.border_control),
                ("accent_text", c.accent_text),
                ("accent_deep", c.accent_deep),
            ] {
                let r = fg.contrast(bg);
                assert!(r >= 3.0, "{name}: {f} on {s} is {r:.2}:1, needs 3");
            }
        }
        // Danger as text appears on the page and on cards.
        for (s, bg) in [("bg", c.bg), ("raised", c.bg_raised)] {
            let r = c.danger.contrast(bg);
            assert!(
                r >= 4.5,
                "{name}: danger text on {s} is {r:.2}:1, needs 4.5"
            );
        }
        // Text on the primary button, top to bottom.
        for (f, fill) in [("accent", c.accent), ("accent_deep", c.accent_deep)] {
            let r = c.on_accent.contrast(fill);
            assert!(r >= 4.5, "{name}: on_accent on {f} is {r:.2}:1, needs 4.5");
        }
        // Disabled text is exempt, but must still read as text and differ
        // from enabled secondary text.
        assert!(c.text_disabled.contrast(c.bg) >= 2.0);
        assert!(c.text_disabled.contrast(c.text_secondary) >= 1.5);
        // The hairline is quiet: visible but well below control borders.
        assert!(c.hairline.contrast(c.bg) < c.border_control.contrast(c.bg));
        // The glass hides most of the scene: what shows through is at
        // most a fifth of the pane.
        assert!(c.glass.pane_alpha >= 0.8, "{name}: pane too clear");
        assert!(c.glass.tint_top < c.glass.tint_bottom);
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
        ] {
            assert_eq!(s % 4.0, 0.0);
        }
        for t in [text::SMALL, text::BODY, text::TITLE, text::DISPLAY] {
            assert_eq!(t.line % 2.0, 0.0);
            assert!(t.line / t.size >= 1.2 && t.line / t.size <= 1.45, "{t:?}");
        }
    }
}
