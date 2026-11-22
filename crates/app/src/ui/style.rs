//! egui styled from the design tokens: our fonts, our colours, our spacing.
//! Nothing of egui's stock look should survive (docs/design.md section 2).

use crate::tokens::{self, Colors, Rgb, radius, size, space, text};
use clairlune_scene::Theme;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke,
    TextStyle, Visuals,
};
use std::sync::Arc;

pub const SEMIBOLD: &str = "plex-semibold";
pub const MEDIUM: &str = "plex-medium";
/// Instrument Serif, for page titles.
pub const SERIF: &str = "serif";
pub const SERIF_ITALIC: &str = "serif-italic";

pub fn c(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.0, rgb.1, rgb.2)
}

pub fn colors(theme: Theme) -> &'static Colors {
    match theme {
        Theme::Light => &tokens::LIGHT,
        Theme::Dark => &tokens::DARK,
    }
}

/// IBM Plex Sans for the UI (three weights), JetBrains Mono for data and
/// labels (the wallpaper's own face), Instrument Serif for page titles.
pub fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::empty();
    let mut add = |name: &str, bytes: &'static [u8]| {
        f.font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(
        "plex",
        include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf"),
    );
    add(
        MEDIUM,
        include_bytes!("../../assets/fonts/IBMPlexSans-Medium.ttf"),
    );
    add(
        SEMIBOLD,
        include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf"),
    );
    add("mono", clairlune_render::atlas::FONT_DATA);
    add(
        SERIF,
        include_bytes!("../../assets/fonts/InstrumentSerif-Regular.ttf"),
    );
    add(
        SERIF_ITALIC,
        include_bytes!("../../assets/fonts/InstrumentSerif-Italic.ttf"),
    );
    f.families
        .insert(FontFamily::Proportional, vec!["plex".into(), "mono".into()]);
    f.families
        .insert(FontFamily::Monospace, vec!["mono".into(), "plex".into()]);
    f.families.insert(
        FontFamily::Name(SEMIBOLD.into()),
        vec![SEMIBOLD.into(), "plex".into()],
    );
    f.families.insert(
        FontFamily::Name(MEDIUM.into()),
        vec![MEDIUM.into(), "plex".into()],
    );
    for name in [SERIF, SERIF_ITALIC] {
        f.families.insert(
            FontFamily::Name(name.into()),
            vec![name.into(), "plex".into()],
        );
    }
    f
}

pub fn body() -> FontId {
    FontId::new(text::BODY.size, FontFamily::Proportional)
}
pub fn body_strong() -> FontId {
    FontId::new(text::BODY.size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn body_medium() -> FontId {
    FontId::new(text::BODY.size, FontFamily::Name(MEDIUM.into()))
}
/// Window and dialog titles: the serif.
pub fn title() -> FontId {
    FontId::new(text::TITLE.size, FontFamily::Name(SERIF.into()))
}
pub fn mono() -> FontId {
    FontId::new(text::SMALL.size, FontFamily::Monospace)
}
/// Page titles: the serif, large.
pub fn display() -> FontId {
    FontId::new(text::DISPLAY.size, FontFamily::Name(SERIF.into()))
}
/// The serif at another size (the wordmark), upright or italic.
pub fn serif(size: f32, italic: bool) -> FontId {
    let name = if italic { SERIF_ITALIC } else { SERIF };
    FontId::new(size, FontFamily::Name(name.into()))
}
/// Group labels: small mono capitals, set apart.
pub fn label() -> FontId {
    FontId::new(11.0, FontFamily::Monospace)
}
/// Letter spacing for [`label`] text, in points.
pub const LABEL_SPACING: f32 = 1.6;

/// A colour at an opacity (0 to 1).
pub fn alpha(rgb: Rgb, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        rgb.0,
        rgb.1,
        rgb.2,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// Applies the theme's tokens to the context.
pub fn apply(ctx: &egui::Context, theme: Theme) {
    let t = colors(theme);
    let mut v = match theme {
        Theme::Light => Visuals::light(),
        Theme::Dark => Visuals::dark(),
    };
    let control = CornerRadius::same(radius::CONTROL as u8);
    v.panel_fill = c(t.bg);
    v.window_fill = c(t.bg_raised);
    v.extreme_bg_color = c(t.bg_raised);
    v.faint_bg_color = c(t.bg_raised);
    v.code_bg_color = c(t.bg_raised);
    v.hyperlink_color = c(t.accent_text);
    v.override_text_color = Some(c(t.text));
    // Glyphs on whole pixels: egui's quarter-pixel placement evens out the
    // spacing but softens small text.
    v.text_options.subpixel_binning = false;
    v.window_corner_radius = CornerRadius::same(radius::PANEL as u8);
    v.menu_corner_radius = control;
    v.window_stroke = Stroke::new(size::HAIRLINE, c(t.hairline));
    v.window_shadow = Shadow::NONE;
    // The one elevation level: popups and the undo toast.
    v.popup_shadow = Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(match theme {
            Theme::Light => 28,
            Theme::Dark => 90,
        }),
    };
    v.selection.bg_fill = c(t.accent);
    v.selection.stroke = Stroke::new(1.0, c(t.on_accent));
    v.text_cursor.stroke = Stroke::new(2.0, c(t.accent_text));
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c(t.bg);
    w.noninteractive.weak_bg_fill = c(t.bg);
    w.noninteractive.bg_stroke = Stroke::new(size::HAIRLINE, c(t.hairline));
    w.noninteractive.fg_stroke = Stroke::new(1.0, c(t.text_secondary));
    // Buttons: a card of glass with a quiet edge; text fields keep the
    // control outline (`extreme_bg_color` and the inactive stroke).
    let g = &t.glass;
    let card = |a: f32| alpha(g.card, a);
    for (state, fill, stroke) in [
        (
            &mut w.inactive,
            card(g.card_alpha * 1.6),
            alpha(t.border_control, 0.55),
        ),
        (
            &mut w.hovered,
            card(g.card_alpha * 2.6),
            alpha(t.border_control, 0.9),
        ),
        (&mut w.active, card(g.card_alpha * 3.2), c(t.accent_text)),
        (&mut w.open, card(g.card_alpha * 3.2), c(t.accent_text)),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, c(t.text));
        state.corner_radius = control;
        state.expansion = 0.0;
    }
    w.noninteractive.corner_radius = control;
    // egui keeps a style per theme and follows the window's theme; pin it to
    // ours and style that one, or the stock look shows through.
    let et = match theme {
        Theme::Light => egui::Theme::Light,
        Theme::Dark => egui::Theme::Dark,
    };
    ctx.set_theme(et);
    ctx.set_visuals_of(et, v);
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (
                TextStyle::Small,
                FontId::new(text::SMALL.size, FontFamily::Proportional),
            ),
            (TextStyle::Body, body()),
            (TextStyle::Button, body()),
            (TextStyle::Heading, title()),
            (TextStyle::Monospace, mono()),
        ]
        .into();
        let sp = &mut s.spacing;
        sp.item_spacing = egui::vec2(space::S2, space::S2);
        sp.button_padding = egui::vec2(space::S3 + 2.0, space::S2 - 1.0);
        sp.interact_size = egui::vec2(space::S7, size::CONTROL);
        sp.indent = space::S4;
        sp.menu_margin = Margin::same(space::S2 as i8);
        sp.window_margin = Margin::same(space::S4 as i8);
        s.animation_time = tokens::motion::BASE as f32 / 1000.0;
    });
}
