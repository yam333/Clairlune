//! The controls every page uses: rows, switches, segmented controls,
//! chips, tiles, buttons and the drawn icons.

use super::*;

/// The small drawn icons.
#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Section(Section),
    Previous,
    Keep,
}

/// Draws an icon in `r` (laid out on a 16 x 16 grid), in strokes.
pub(crate) fn icon(p: &Painter, icon: Icon, r: Rect, ink: Color32) {
    let k = r.width() / 16.0;
    let at = |x: f32, y: f32| pos2(r.min.x + x * k, r.min.y + y * k);
    let st = Stroke::new(1.4 * k, ink);
    let line = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            pts.iter().map(|&(x, y)| at(x, y)).collect(),
            st,
        ));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::closed_line(
            pts.iter().map(|&(x, y)| at(x, y)).collect(),
            st,
        ));
    };
    let arc = |cx: f32, cy: f32, rad: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=12)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / 12.0).to_radians();
                at(cx + rad * a.cos(), cy + rad * a.sin())
            })
            .collect()
    };
    match icon {
        Icon::Section(Section::Rotation) => {
            p.add(egui::Shape::line(arc(8.0, 8.0, 5.3, 200.0, 330.0), st));
            line(&[(10.6, 2.9), (12.6, 5.4), (9.6, 5.9)]);
            p.add(egui::Shape::line(arc(8.0, 8.0, 5.3, 20.0, 150.0), st));
            line(&[(5.4, 13.1), (3.4, 10.6), (6.4, 10.1)]);
        }
        Icon::Section(Section::Display) => {
            p.rect_stroke(
                Rect::from_min_max(at(1.8, 2.8), at(14.2, 11.2)),
                1.6 * k,
                st,
                egui::StrokeKind::Middle,
            );
            line(&[(5.8, 14.0), (10.2, 14.0)]);
            line(&[(8.0, 11.2), (8.0, 14.0)]);
        }
        Icon::Section(Section::Live) => {
            line(&[
                (1.5, 8.5),
                (4.3, 8.5),
                (5.9, 4.3),
                (8.9, 12.5),
                (10.6, 8.5),
                (14.5, 8.5),
            ]);
        }
        Icon::Section(Section::Buddy) => {
            p.rect_stroke(
                Rect::from_min_max(at(3.0, 3.5), at(13.0, 10.5)),
                0.8 * k,
                st,
                egui::StrokeKind::Middle,
            );
            for x in [4.6, 6.8, 9.2, 11.4] {
                line(&[(x, 10.5), (x, 13.1)]);
            }
            line(&[(1.4, 6.8), (3.0, 6.8)]);
            line(&[(13.0, 6.8), (14.6, 6.8)]);
            line(&[(6.0, 6.2), (6.0, 7.4)]);
            line(&[(10.0, 6.2), (10.0, 7.4)]);
        }
        Icon::Section(Section::Content) => {
            closed(&[
                (1.2, 13.2),
                (5.6, 6.6),
                (8.7, 10.8),
                (10.7, 8.2),
                (14.8, 13.2),
            ]);
            p.circle_stroke(at(11.6, 4.2), 1.5 * k, st);
        }
        Icon::Section(Section::Archive) => {
            p.rect_stroke(
                Rect::from_min_max(at(1.8, 2.6), at(14.2, 5.8)),
                1.0 * k,
                st,
                egui::StrokeKind::Middle,
            );
            line(&[(3.0, 5.8), (3.0, 13.2), (13.0, 13.2), (13.0, 5.8)]);
            line(&[(6.4, 8.8), (9.6, 8.8)]);
        }
        Icon::Section(Section::Export) => {
            line(&[(8.0, 1.8), (8.0, 10.2)]);
            line(&[(4.8, 5.0), (8.0, 1.8), (11.2, 5.0)]);
            line(&[(2.6, 10.2), (2.6, 13.8), (13.4, 13.8), (13.4, 10.2)]);
        }
        Icon::Section(Section::General) => {
            line(&[(2.4, 4.0), (8.6, 4.0)]);
            line(&[(12.4, 4.0), (13.6, 4.0)]);
            line(&[(2.4, 8.0), (3.8, 8.0)]);
            line(&[(7.4, 8.0), (13.6, 8.0)]);
            line(&[(2.4, 12.0), (9.6, 12.0)]);
            for (x, y) in [(10.5, 4.0), (5.6, 8.0), (11.4, 12.0)] {
                p.circle_stroke(at(x, y), 1.6 * k, st);
            }
        }
        Icon::Section(Section::About) => {
            p.circle_stroke(at(8.0, 8.0), 6.2 * k, st);
            line(&[(8.0, 7.2), (8.0, 11.6)]);
            p.circle_filled(at(8.0, 4.8), 0.95 * k, ink);
        }
        Icon::Previous => {
            line(&[(5.5, 3.5), (2.5, 6.5), (5.5, 9.5)]);
            let mut back = vec![at(2.5, 6.5), at(9.5, 6.5)];
            back.extend(arc(9.5, 10.5, 4.0, -90.0, 90.0));
            back.push(at(7.0, 14.5));
            p.add(egui::Shape::line(back, st));
        }
        Icon::Keep => closed(&[
            (4.0, 2.5),
            (12.0, 2.5),
            (12.0, 13.5),
            (8.0, 10.8),
            (4.0, 13.5),
        ]),
    }
}

/// The app's mark (crate::icon): a square over a circle, its lower left corner
/// the circle's centre.
pub(crate) fn mark(p: &Painter, r: Rect, circle: Color32, square: Color32) {
    let k = r.width() / 32.0;
    let ppp = p.pixels_per_point();
    let px = |v: f32| (v * ppp).round() / ppp;
    let at = pos2(px(r.min.x + 14.0 * k), px(r.min.y + 18.0 * k));
    let side = px(16.0 * k);
    p.circle_filled(at, 12.0 * k, circle);
    p.rect_filled(
        Rect::from_min_max(at - vec2(0.0, side), at + vec2(side, 0.0)),
        2.0 * k,
        square,
    );
}

/// A page with nothing on it yet: the mark, and what will be here.
pub(crate) fn empty_page(ui: &mut Ui, theme: Theme, title: &str, what: &str) {
    let t = colors(theme);
    ui.add_space(space::S7);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
        mark(ui.painter(), rect, c(t.text_secondary), c(t.accent_deep));
        ui.add_space(space::S3);
        ui.label(RichText::new(title).font(style::body_strong()));
        ui.label(RichText::new(what).color(c(t.text_secondary)));
    });
}

/// A square button with a drawn icon.
pub(crate) fn icon_button(ui: &mut Ui, theme: Theme, which: Icon, label: &str) -> egui::Response {
    let t = colors(theme);
    let side = size::BUTTON_PRIMARY;
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let g = &t.glass;
    let fill = if resp.hovered() {
        g.card_alpha * 3.0
    } else {
        g.card_alpha * 1.8
    };
    let p = ui.painter();
    p.rect_filled(rect, radius::CONTROL, alpha(g.card, fill));
    p.rect_stroke(
        rect,
        radius::CONTROL,
        Stroke::new(1.0, alpha(t.border_control, 0.45)),
        egui::StrokeKind::Inside,
    );
    let ink = if ui.is_enabled() {
        c(t.text)
    } else {
        c(t.text_disabled)
    };
    icon(
        p,
        which,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        ink,
    );
    if resp.has_focus() {
        focus_ring(ui, theme, rect, radius::CONTROL);
    }
    resp.on_hover_text(label)
}

pub(crate) fn focus_ring(ui: &Ui, theme: Theme, rect: Rect, r: f32) {
    ui.painter().rect_stroke(
        rect.expand(size::FOCUS_OFFSET),
        r + size::FOCUS_OFFSET,
        Stroke::new(size::FOCUS_RING, c(colors(theme).accent_text)),
        egui::StrokeKind::Outside,
    );
}

pub(crate) fn primary_button(ui: &mut Ui, theme: Theme, label: &str) -> egui::Response {
    primary_button_wide(ui, theme, label, 0.0)
}

/// The primary action: ember glass, at least `min_width` wide.
pub(crate) fn primary_button_wide(
    ui: &mut Ui,
    theme: Theme,
    label: &str,
    min_width: f32,
) -> egui::Response {
    let t = colors(theme);
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), style::body_strong(), c(t.on_accent));
    let w = (galley.size().x + 2.0 * space::S4).max(min_width);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, size::BUTTON_PRIMARY), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let enabled = ui.is_enabled();
    let fade = |col: Color32| {
        if enabled {
            col
        } else {
            col.gamma_multiply(0.45)
        }
    };
    let p = ui.painter();
    let fill = if resp.hovered() && enabled {
        t.accent_deep
    } else {
        t.accent
    };
    p.rect_filled(rect, radius::CONTROL, fade(c(fill)));
    // Glass: light along the top edge, the deeper ember along the bottom.
    p.hline(
        rect.x_range().shrink(radius::CONTROL),
        rect.min.y + 0.5,
        Stroke::new(1.0, fade(Color32::from_white_alpha(90))),
    );
    p.hline(
        rect.x_range().shrink(radius::CONTROL),
        rect.max.y - 0.5,
        Stroke::new(1.0, fade(c(t.accent_deep))),
    );
    let pos = rect.center() - galley.size() / 2.0;
    p.galley(pos, galley, c(t.on_accent));
    if resp.has_focus() {
        focus_ring(ui, theme, rect, radius::CONTROL);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(crate) fn hairline(ui: &mut Ui, color: crate::tokens::Rgb) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c(color));
}

pub(crate) fn group(ui: &mut Ui, title: &str) {
    ui.add_space(space::S3);
    let ink = ui.visuals().widgets.noninteractive.fg_stroke.color;
    ui.label(
        RichText::new(title.to_uppercase())
            .font(style::label())
            .extra_letter_spacing(style::LABEL_SPACING)
            .color(ink),
    );
    ui.add_space(space::S2);
}

/// A time field's width: all of "00:00" with the field's margins.
pub(crate) const CLOCK_W: f32 = 76.0;

/// A text field whose text reaches the settings once it's done (focus
/// leaves, or Enter) and `valid` takes it: a half-typed value doesn't
/// change the settings, and the wallpaper, on every key. Returns the text
/// shown, which may still be wrong.
pub(crate) fn settled_text(
    ui: &mut Ui,
    name: &str,
    value: &mut String,
    hint: &str,
    width: f32,
    valid: impl Fn(&str) -> bool,
) -> String {
    let id = egui::Id::unique(("settled-text", name));
    let mut text = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| value.clone());
    let r = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id.with("edit"))
            .hint_text(hint)
            .desired_width(width),
    );
    if r.lost_focus() && valid(&text) {
        value.clone_from(&text);
        ui.data_mut(|d| d.remove::<String>(id));
    } else if r.has_focus() || r.lost_focus() {
        // Being typed, or left wrong: kept here until it's right.
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    text
}

pub(crate) fn row(
    ui: &mut Ui,
    theme: Theme,
    name: &str,
    description: &str,
    control: impl FnOnce(&mut Ui),
) {
    let t = colors(theme);
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).font(style::body_medium()));
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
    // A setting that says it all in its name has no line under it.
    if !description.is_empty() {
        ui.label(RichText::new(description).color(c(t.text_secondary)));
    }
    ui.add_space(space::S4);
}

pub(crate) fn toggle(ui: &mut Ui, theme: Theme, on: &mut bool, label: &str) -> egui::Response {
    let t = colors(theme);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(40.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *on, label));
    let k = ui.ctx().animate_bool_responsive(resp.id, *on);
    let enabled = ui.is_enabled();
    let fade = |col: Color32| {
        if enabled {
            col
        } else {
            col.gamma_multiply(0.4)
        }
    };
    let p = ui.painter();
    let off_fill = alpha(t.border_control, 0.28);
    let track = if *on { c(t.accent_deep) } else { off_fill };
    p.rect_filled(rect, 12.0, fade(track));
    if !*on {
        p.rect_stroke(
            rect,
            12.0,
            Stroke::new(1.0, fade(alpha(t.border_control, 0.9))),
            egui::StrokeKind::Inside,
        );
    }
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, k);
    let at = pos2(x, rect.center().y);
    p.circle_filled(
        at + vec2(0.0, 1.0),
        9.0,
        fade(Color32::from_black_alpha(50)),
    );
    p.circle_filled(at, 9.0, fade(Color32::WHITE));
    if resp.has_focus() {
        focus_ring(ui, theme, rect, 12.0);
    }
    resp
}

pub(crate) fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: Theme,
    value: &mut T,
    options: &[(T, &str, bool)],
) -> bool {
    let t = colors(theme);
    let g = &t.glass;
    let pad = space::S3;
    let galleys: Vec<_> = options
        .iter()
        .map(|(v, label, enabled)| {
            let ink = if *value == *v {
                t.text
            } else if *enabled {
                t.text_secondary
            } else {
                t.text_disabled
            };
            ui.painter()
                .layout_no_wrap((*label).to_string(), style::body(), c(ink))
        })
        .collect();
    let inner: f32 = galleys.iter().map(|g| g.size().x + 2.0 * pad).sum();
    // This control's own ids, whatever its labels: rows that read the same
    // (Live / Still frame, a monitor each) mustn't answer each other's clicks.
    let base = ui.next_auto_id();
    let (track, _) = ui.allocate_exact_size(vec2(inner + 6.0, size::CONTROL + 2.0), Sense::hover());
    let groove = match theme {
        Theme::Dark => Color32::from_black_alpha(80),
        Theme::Light => alpha(t.text, 0.06),
    };
    ui.painter()
        .rect_filled(track, radius::CONTROL + 1.0, groove);
    ui.painter().rect_stroke(
        track,
        radius::CONTROL + 1.0,
        Stroke::new(1.0, alpha(t.border_control, 0.35)),
        egui::StrokeKind::Inside,
    );
    let mut changed = false;
    let mut x = track.min.x + 3.0;
    for (i, ((v, label, enabled), galley)) in options.iter().zip(galleys).enumerate() {
        let w = galley.size().x + 2.0 * pad;
        let seg = Rect::from_min_size(pos2(x, track.min.y + 3.0), vec2(w, track.height() - 6.0));
        x += w;
        let on = *value == *v;
        let sense = if *enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let resp = ui.interact(seg, base.with(("segment", i, *label)), sense);
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::RadioButton, *enabled, on, *label)
        });
        let p = ui.painter();
        if on {
            let bead = match theme {
                Theme::Dark => alpha(g.card, g.chosen * 1.2),
                Theme::Light => Color32::from_white_alpha(235),
            };
            p.rect_filled(seg, radius::CONTROL - 2.0, bead);
            p.hline(
                seg.x_range().shrink(radius::CONTROL - 2.0),
                seg.min.y + 0.5,
                Stroke::new(1.0, highlight(theme)),
            );
        } else if resp.hovered() && *enabled {
            p.rect_filled(seg, radius::CONTROL - 2.0, alpha(g.card, g.chosen * 0.6));
        }
        p.galley(seg.center() - galley.size() / 2.0, galley, c(t.text));
        if resp.has_focus() {
            focus_ring(ui, theme, seg, radius::CONTROL - 2.0);
        }
        if resp.clicked() && !on && *enabled {
            *value = *v;
            changed = true;
        }
    }
    changed
}

pub(crate) fn filter_checks(ui: &mut Ui, list: &mut Vec<String>, items: &[Item], always: &[&str]) {
    filter_chips(ui, list, items, always, |_| None);
}

/// [`filter_checks`] with a glyph before each chip's name where `glyph`
/// gives one (weather: what the scene draws for it).
pub(crate) fn filter_chips(
    ui: &mut Ui,
    list: &mut Vec<String>,
    items: &[Item],
    always: &[&str],
    glyph: fn(&str) -> Option<&'static str>,
) {
    let all: Vec<String> = items.iter().map(|(id, _, _)| id.to_string()).collect();
    let mut on: Vec<bool> = items
        .iter()
        .map(|(id, _, _)| list.is_empty() || list.iter().any(|x| x == id))
        .collect();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
        for (i, (id, label, hint)) in items.iter().enumerate() {
            let last_one = on[i] && on.iter().filter(|x| **x).count() == 1;
            let mut r = chip(ui, on[i], glyph(id), label, !last_one);
            if !hint.is_empty() {
                r = r.on_hover_text(*hint);
            }
            if r.clicked() {
                on[i] = !on[i];
                changed = true;
            }
        }
        if on.iter().any(|o| !o) && all_link(ui) {
            on.fill(true);
            changed = true;
        }
    });
    if changed {
        *list = picked(&all, &on, always);
    }
}

/// "All", a link turning everything back on; true when clicked.
pub(crate) fn all_link(ui: &mut Ui) -> bool {
    let link = ui.add(
        egui::Label::new(
            RichText::new("All")
                .color(ui.visuals().hyperlink_color)
                .font(style::body()),
        )
        .sense(egui::Sense::click()),
    );
    link.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A filter list from the items' on/off states: empty when all are on.
pub(crate) fn picked(all: &[String], on: &[bool], always: &[&str]) -> Vec<String> {
    let chosen: Vec<String> = all
        .iter()
        .zip(on)
        .filter(|(_, o)| **o)
        .map(|(id, _)| id.clone())
        .collect();
    if chosen.len() == all.len() {
        Vec::new()
    } else {
        always.iter().map(|s| s.to_string()).chain(chosen).collect()
    }
}

/// What an empty list of choices stands for.
#[derive(Clone, Copy)]
pub(crate) enum Empty {
    /// Every one.
    All,
    /// This one alone (the default).
    Only(&'static str),
}

/// Choices as tiles: a small scene of each (drawn once by the app, as
/// `<key><id>.png`; a quiet placeholder until it's there), its name, and a
/// tick when on. One that's off stays in view, dimmed. At least one stays on.
pub(crate) fn pic_tiles(
    ui: &mut Ui,
    theme: Theme,
    list: &mut Vec<String>,
    items: &[Item],
    empty: Empty,
    key: &str,
    lands: &mut LandThumbs,
) {
    let t = colors(theme);
    let g = &t.glass;
    let all: Vec<String> = items.iter().map(|(id, _, _)| id.to_string()).collect();
    let mut on: Vec<bool> = items
        .iter()
        .map(|(id, _, _)| match (list.is_empty(), empty) {
            (true, Empty::All) => true,
            (true, Empty::Only(only)) => *id == only,
            (false, _) => list.iter().any(|x| x == id),
        })
        .collect();
    let mut changed = false;
    let gap = space::S3;
    let width = ui.available_width();
    let cols = if width >= 560.0 { 4 } else { 3 };
    let tile_w = ((width - gap * (cols - 1) as f32) / cols as f32).floor();
    let pic_h = (tile_w - 12.0) * 9.0 / 16.0;
    let tile_h = 6.0 + pic_h + 32.0;
    for row in items.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (id, label, hint) in row {
                let i = items.iter().position(|(x, _, _)| x == id).unwrap_or(0);
                let last_one = on[i] && on.iter().filter(|x| **x).count() == 1;
                let sense = if last_one {
                    Sense::hover()
                } else {
                    Sense::click()
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(tile_w, tile_h), sense);
                resp.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::CheckBox, !last_one, on[i], *label)
                });
                let lift = if resp.hovered() && !last_one {
                    2.0
                } else {
                    0.0
                };
                let r = rect.translate(vec2(0.0, -lift));
                let p = ui.painter();
                p.rect_filled(r, radius::CONTROL + 2.0, alpha(g.card, g.card_alpha * 1.3));
                p.rect_stroke(
                    r,
                    radius::CONTROL + 2.0,
                    Stroke::new(1.0, alpha(t.border_control, if on[i] { 0.35 } else { 0.2 })),
                    egui::StrokeKind::Inside,
                );
                let pic = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(tile_w - 12.0, pic_h));
                match lands.get(ui.ctx(), &format!("{key}{id}")) {
                    Some(tex) => {
                        // Off is dimmed; less so where the tile is a colour.
                        let tint = match (on[i], empty) {
                            (true, _) => Color32::WHITE,
                            (false, Empty::All) => {
                                Color32::from_rgba_unmultiplied(120, 120, 120, 150)
                            }
                            (false, Empty::Only(_)) => {
                                Color32::from_rgba_unmultiplied(190, 190, 190, 215)
                            }
                        };
                        egui::Image::new(tex)
                            .uv(cover_uv(tex.size_vec2(), pic.size()))
                            .corner_radius(7)
                            .tint(tint)
                            .paint_at(ui, pic);
                    }
                    None => {
                        let p = ui.painter();
                        p.rect_filled(pic, 7.0, alpha(g.card, g.card_alpha * 2.0));
                        p.text(
                            pic.center(),
                            Align2::CENTER_CENTER,
                            "drawing…",
                            style::mono(),
                            alpha(t.text_secondary, 0.7),
                        );
                    }
                }
                let p = ui.painter();
                let ink = if on[i] {
                    c(t.text)
                } else {
                    c(t.text_secondary)
                };
                let name = p.layout_job(one_line(label, style::body(), ink, tile_w - 44.0));
                p.galley(pos2(r.min.x + 9.0, pic.max.y + 8.0), name, ink);
                let tick = pos2(r.max.x - 16.0, pic.max.y + 16.0);
                if on[i] {
                    p.circle_filled(tick, 9.0, c(t.accent_deep));
                    p.add(egui::Shape::line(
                        vec![
                            tick + vec2(-4.0, 0.2),
                            tick + vec2(-1.2, 3.0),
                            tick + vec2(4.0, -3.2),
                        ],
                        Stroke::new(1.8, c(t.on_accent)),
                    ));
                } else {
                    p.circle_stroke(tick, 8.5, Stroke::new(1.5, alpha(t.border_control, 0.8)));
                }
                if resp.has_focus() {
                    focus_ring(ui, theme, r, radius::CONTROL + 2.0);
                }
                let resp = if hint.is_empty() {
                    resp
                } else {
                    resp.on_hover_text(*hint)
                };
                if resp.clicked() {
                    on[i] = !on[i];
                    changed = true;
                }
            }
        });
        ui.add_space(gap);
    }
    if on.iter().any(|o| !o) && all_link(ui) {
        on.fill(true);
        changed = true;
    }
    if changed {
        *list = match empty {
            Empty::All => picked(&all, &on, &[]),
            Empty::Only(only) => {
                let chosen: Vec<String> = all
                    .iter()
                    .zip(&on)
                    .filter(|(_, o)| **o)
                    .map(|(id, _)| id.clone())
                    .collect();
                if chosen == [only] { Vec::new() } else { chosen }
            }
        };
    }
}

/// The glyph each weather draws on the wallpaper.
pub(crate) fn weather_glyph(id: &str) -> Option<&'static str> {
    Some(match id {
        "snow" | "blizzard" => "*",
        "rain" => "/",
        "fireflies" => "·",
        "leaves" => ",",
        "dust" | "ash" => ".",
        "petals" => "'",
        "fog" => "~",
        "sparkle" => "+",
        "confetti" => "%",
        "birds" => "v",
        "butterflies" => "x",
        "seeds" => "*",
        "shooting_stars" => "\\",
        _ => return None,
    })
}

pub(crate) fn chip(
    ui: &mut Ui,
    on: bool,
    glyph: Option<&str>,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let t = if dark {
        &crate::tokens::DARK
    } else {
        &crate::tokens::LIGHT
    };
    let g = &t.glass;
    let text = if on { c(t.text) } else { c(t.text_secondary) };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), style::body(), text);
    let lead = match glyph {
        Some(_) => 16.0,
        None if on => 15.0,
        None => 0.0,
    };
    let pad = space::S3 + 1.0;
    let h = 30.0;
    let w = galley.size().x + 2.0 * pad + lead;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, label));
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let round = egui::CornerRadius::same((h / 2.0) as u8);
        let hovered = resp.hovered() && enabled;
        if on {
            let fill = if dark {
                alpha(g.card, g.chosen * if hovered { 1.4 } else { 1.0 })
            } else {
                Color32::from_white_alpha(if hovered { 255 } else { 215 })
            };
            p.rect_filled(rect, round, fill);
            p.rect_stroke(
                rect,
                round,
                Stroke::new(1.0, alpha(t.text, if dark { 0.22 } else { 0.16 })),
                egui::StrokeKind::Inside,
            );
            p.hline(
                rect.x_range().shrink(h / 2.0),
                rect.min.y + 0.5,
                Stroke::new(
                    1.0,
                    highlight(if dark { Theme::Dark } else { Theme::Light }),
                ),
            );
        } else {
            if hovered {
                p.rect_filled(rect, round, alpha(g.card, g.chosen * 0.6));
            }
            p.rect_stroke(
                rect,
                round,
                Stroke::new(1.0, alpha(t.text, 0.12)),
                egui::StrokeKind::Inside,
            );
        }
        let mut x = rect.left() + pad;
        let cy = rect.center().y;
        match glyph {
            Some(gl) => {
                let ink = if on {
                    c(t.accent_text)
                } else {
                    c(t.text_disabled)
                };
                p.text(
                    pos2(x + 4.0, cy),
                    Align2::CENTER_CENTER,
                    gl,
                    style::mono(),
                    ink,
                );
                x += lead;
            }
            None if on => {
                // A small check mark, drawn (no font has to have one).
                let pts = vec![
                    pos2(x, cy),
                    pos2(x + 3.5, cy + 3.5),
                    pos2(x + 9.0, cy - 3.5),
                ];
                p.add(egui::Shape::line(pts, Stroke::new(1.6, c(t.accent_text))));
                x += lead;
            }
            None => {}
        }
        p.galley(pos2(x, cy - galley.size().y / 2.0), galley, text);
    }
    resp.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::NotAllowed
    })
}
