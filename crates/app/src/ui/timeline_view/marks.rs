//! The editor's drawn marks and small controls: glyphs, icon buttons,
//! day chips, hatching, tiles.

use super::*;

/// The random scene's tile: a landscape in miniature, drawn in rows of dots
/// like the wallpapers, with a die on it.
pub(crate) fn random_tile(p: &egui::Painter, theme: Theme, r: Rect, hot: bool) {
    let t = colors(theme);
    p.rect_filled(r, radius::CONTROL, c(t.bg_raised));
    let clip = p.with_clip_rect(r.shrink(2.0));
    let h = |k: u32| (k.wrapping_mul(2_654_435_761) >> 8) as f32 / (1u32 << 24) as f32;
    let rows = 10;
    for row in 0..rows {
        let y = r.top() + 6.0 + row as f32 * (r.height() - 12.0) / (rows - 1) as f32;
        // Sky above, ground below: denser lower down.
        let dense = 0.1 + 0.75 * (row as f32 / rows as f32).powf(1.4);
        let mut x = r.left() + 4.0;
        let mut k = row as u32 * 131 + 7;
        while x < r.right() - 4.0 {
            k += 1;
            if h(k) < dense {
                clip.rect_filled(
                    Rect::from_min_size(pos2(x, y), vec2(2.0, 1.5)),
                    0.0,
                    alpha(t.text_secondary, 0.5),
                );
            }
            x += 5.0;
        }
    }
    let die = Rect::from_center_size(r.center(), vec2(30.0, 30.0));
    p.rect_filled(die, 7.0, c(t.bg));
    p.rect_stroke(die, 7.0, Stroke::new(1.4, c(t.text)), StrokeKind::Inside);
    for (x, y) in [
        (-1.0, -1.0),
        (1.0, 1.0),
        (0.0, 0.0),
        (1.0, -1.0),
        (-1.0, 1.0),
    ] {
        p.circle_filled(die.center() + vec2(x * 7.0, y * 7.0), 2.2, c(t.text));
    }
    p.rect_stroke(
        r,
        radius::CONTROL,
        Stroke::new(size::HAIRLINE, c(t.border_control)),
        StrokeKind::Inside,
    );
    if hot {
        lifted(p, theme, r);
    }
}

/// A tile under the pointer: outlined, with a grip in its corner.
pub(crate) fn lifted(p: &egui::Painter, theme: Theme, r: Rect) {
    let t = colors(theme);
    p.rect_stroke(
        r,
        radius::CONTROL,
        Stroke::new(1.6, c(t.accent_text)),
        StrokeKind::Inside,
    );
    let g = Rect::from_min_size(pos2(r.right() - 22.0, r.top() + 5.0), vec2(17.0, 17.0));
    p.rect_filled(g, 5.0, c(t.bg).gamma_multiply(0.85));
    glyph(p, Glyph::Grip, g.shrink(3.0), c(t.text));
}

/// Where saved wallpapers will be: an empty frame with the library's mark.
pub(crate) fn empty_library(p: &egui::Painter, theme: Theme, r: Rect) {
    let t = colors(theme);
    let stroke = Stroke::new(1.2, alpha(t.text_secondary, 0.5));
    let (a, b, cc, d) = (
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    );
    for (p0, p1) in [(a, b), (b, cc), (cc, d), (d, a)] {
        p.extend(egui::Shape::dashed_line(&[p0, p1], stroke, 5.0, 4.0));
    }
    glyph(
        p,
        Glyph::Bookmark,
        Rect::from_center_size(r.center(), vec2(22.0, 22.0)),
        alpha(t.text_secondary, 0.85),
    );
}

/// A day as three letters.
pub(crate) fn short_day(d: usize) -> &'static str {
    let name = day_name(d);
    &name[..3.min(name.len())]
}

/// A round chip for a day: on, off, or marked as about to be replaced.
pub(crate) fn day_chip(
    ui: &mut Ui,
    theme: Theme,
    label: &str,
    on: bool,
    replaced: bool,
    enabled: bool,
) -> egui::Response {
    let t = colors(theme);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 28.0), sense);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, label));
    let p = ui.painter();
    let fill = match (on, enabled) {
        (true, true) => c(t.accent_deep),
        (true, false) => alpha(t.accent_deep, 0.4),
        _ if resp.hovered() => alpha(t.text, 0.10),
        _ => alpha(t.text, 0.05),
    };
    p.rect_filled(rect, 14.0, fill);
    if replaced {
        p.rect_stroke(
            rect,
            14.0,
            Stroke::new(1.6, c(t.danger)),
            StrokeKind::Inside,
        );
    }
    let ink = if on {
        c(t.on_accent)
    } else if enabled {
        c(t.text)
    } else {
        c(t.text_disabled)
    };
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(text::SMALL.size, FontFamily::Proportional),
        ink,
    );
    if resp.has_focus() {
        p.rect_stroke(
            rect.expand(2.0),
            16.0,
            Stroke::new(size::FOCUS_RING, c(t.accent_text)),
            StrokeKind::Outside,
        );
    }
    resp
}

/// The little drawn marks the editor uses instead of words.
#[derive(Clone, Copy)]
pub(crate) enum Glyph {
    Plus,
    Copy,
    Clear,
    Undo,
    Earlier,
    Later,
    Bookmark,
    Warning,
    Grip,
}

/// Draws a mark on a 16-unit grid scaled into `r`.
pub(crate) fn glyph(p: &egui::Painter, g: Glyph, r: Rect, ink: egui::Color32) {
    let k = r.width() / 16.0;
    let at = |x: f32, y: f32| pos2(r.min.x + x * k, r.min.y + y * k);
    let st = Stroke::new(1.5 * k.max(0.8), ink);
    let line = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            pts.iter().map(|&(x, y)| at(x, y)).collect(),
            st,
        ));
    };
    match g {
        Glyph::Plus => {
            line(&[(8.0, 2.5), (8.0, 13.5)]);
            line(&[(2.5, 8.0), (13.5, 8.0)]);
        }
        Glyph::Copy => {
            p.rect_stroke(
                Rect::from_min_max(at(5.0, 5.0), at(14.0, 14.0)),
                1.5 * k,
                st,
                StrokeKind::Middle,
            );
            line(&[(2.0, 10.5), (2.0, 2.0), (10.5, 2.0)]);
        }
        Glyph::Clear => {
            line(&[(3.5, 3.5), (12.5, 12.5)]);
            line(&[(12.5, 3.5), (3.5, 12.5)]);
        }
        Glyph::Undo => {
            line(&[(5.5, 2.5), (2.5, 5.5), (5.5, 8.5)]);
            let mut back = vec![at(2.5, 5.5), at(9.5, 5.5)];
            back.extend((0..=10).map(|i| {
                let a = (-90.0 + 180.0 * i as f32 / 10.0_f32).to_radians();
                at(9.5 + 4.0 * a.cos(), 9.5 + 4.0 * a.sin())
            }));
            back.push(at(6.0, 13.5));
            p.add(egui::Shape::line(back, st));
        }
        Glyph::Earlier => line(&[(10.0, 3.0), (5.0, 8.0), (10.0, 13.0)]),
        Glyph::Later => line(&[(6.0, 3.0), (11.0, 8.0), (6.0, 13.0)]),
        Glyph::Bookmark => {
            p.add(egui::Shape::closed_line(
                [
                    (4.0, 2.5),
                    (12.0, 2.5),
                    (12.0, 13.5),
                    (8.0, 10.8),
                    (4.0, 13.5),
                ]
                .iter()
                .map(|&(x, y)| at(x, y))
                .collect(),
                st,
            ));
        }
        Glyph::Warning => {
            p.add(egui::Shape::closed_line(
                vec![at(8.0, 1.8), at(14.8, 14.0), at(1.2, 14.0)],
                st,
            ));
            line(&[(8.0, 6.0), (8.0, 9.6)]);
            p.circle_filled(at(8.0, 11.8), 0.9 * k, ink);
        }
        Glyph::Grip => {
            for y in [4.0, 8.0, 12.0] {
                for x in [5.5, 10.5] {
                    p.circle_filled(at(x, y), 1.3 * k, ink);
                }
            }
        }
    }
}

/// A small button that is only its mark; the words are in its tooltip.
/// `faint` keeps it quiet until its row is under the pointer.
pub(crate) fn icon_btn(
    ui: &mut Ui,
    theme: Theme,
    g: Glyph,
    tip: &str,
    enabled: bool,
    faint: bool,
) -> egui::Response {
    let t = colors(theme);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
    let hovered = enabled && resp.hovered();
    let p = ui.painter();
    if hovered {
        p.rect_filled(rect, 7.0, alpha(t.text, 0.09));
    }
    let ink = if !enabled {
        alpha(t.text_secondary, 0.3)
    } else if hovered {
        c(t.text)
    } else if faint {
        alpha(t.text_secondary, 0.45)
    } else {
        c(t.text_secondary)
    };
    glyph(p, g, rect.shrink(6.0), ink);
    if resp.has_focus() {
        p.rect_stroke(
            rect,
            7.0,
            Stroke::new(size::FOCUS_RING, c(t.accent_text)),
            StrokeKind::Inside,
        );
    }
    resp.on_hover_text(tip)
}

/// Diagonal lines over a stretch: the day runs itself there (new scenes
/// on the Generate schedule).
pub(crate) fn hatch(p: &egui::Painter, r: Rect, ink: egui::Color32) {
    let clip = p.with_clip_rect(r.shrink(1.5));
    let mut x = r.left() - r.height();
    while x < r.right() {
        clip.line_segment(
            [pos2(x, r.bottom()), pos2(x + r.height(), r.top())],
            Stroke::new(1.0, ink),
        );
        x += 7.0;
    }
}

pub(crate) fn selected_ring(painter: &egui::Painter, theme: Theme, r: Rect) {
    painter.rect_stroke(
        r,
        radius::CONTROL,
        Stroke::new(size::FOCUS_RING, c(colors(theme).accent)),
        StrokeKind::Inside,
    );
}

/// A small label on a scrim, readable over any wallpaper.
pub(crate) fn pill(painter: &egui::Painter, theme: Theme, at: egui::Pos2, label: &str) -> Rect {
    let t = colors(theme);
    let galley = painter.layout_no_wrap(label.to_string(), style::mono(), c(t.text));
    let r = Rect::from_min_size(at, galley.size() + vec2(8.0, 2.0));
    painter.rect_filled(r, 4.0, c(t.bg).gamma_multiply(0.88));
    painter.galley(r.min + vec2(4.0, 1.0), galley, c(t.text));
    r
}
