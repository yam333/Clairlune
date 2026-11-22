//! The timeline editor (PLAN.md section 9b, docs/design.md section 4): a
//! 24-hour bar for every day (one in the single layout, seven in the weekly
//! one), with slots as blocks showing their wallpaper. Saved wallpapers sit on
//! the left, ready to drag onto a day; the selected slot's choices are on the
//! right. Every drag has a keyboard and a button equivalent (WCAG 2.5.7):
//! typing a start or Earlier/Later for dragging an edge, Add for
//! double-clicking, and the slot's own picker for dropping a wallpaper.

use super::style::{self, alpha, c, colors};
use super::view::{self, UiAction, filter_checks, group, hairline, primary_button, segmented};
use crate::library::{BuddyChoice, DisplayChoice, Item, ThemeChoice};
use crate::settings::RotationMode;
use crate::timeline::{
    self, Carried, Content, DAY_MINUTES, Layout as Days, Order, Slot, Timeline, day_name,
    format_time, snap,
};
use crate::tokens::{radius, size, space, text};
use clairlune_scene::Theme;
use egui::{
    Align, Align2, CursorIcon, FontFamily, FontId, Id, Key, Layout, Modifiers, PopupCloseBehavior,
    Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use std::path::Path;

/// What can be dropped on a day.
#[derive(Clone, Debug)]
enum Payload {
    Item(String),
    Random,
}

const UNDO_STEPS: usize = 50;
/// How close to a slot's left edge a press grabs it.
const EDGE_GRAB: f32 = 6.0;
/// The day names and their buttons, left of the bars.
const LABEL_W: f32 = 132.0;
const SIDEBAR_W: f32 = 184.0;
const INSPECTOR_W: f32 = 304.0;

#[derive(Default)]
pub struct TimelineState {
    /// The selected slot, as (day, index) with days counted as shown.
    selected: Option<(usize, usize)>,
    /// A slot start being dragged: (day, index, start so far).
    drag: Option<(usize, usize, u16)>,
    /// Earlier versions for Undo, the latest last.
    undo: Vec<Timeline>,
    /// Going from weekly to single: the day to keep, while asking.
    keep_day: Option<usize>,
    /// The Copy to... picker's ticks.
    copy_to: [bool; 7],
    /// The start field's text, and the slot and start it was filled from.
    start_text: String,
    start_for: Option<(usize, usize, u16)>,
    /// The pool tag field's text, and the slot it belongs to.
    tag_text: String,
    tag_for: Option<(usize, usize)>,
    pub thumbs: super::Thumbs,
    /// Where the day bars and the random scene were drawn last frame.
    bars: Vec<Rect>,
    random_chip: Option<Rect>,
}

impl TimelineState {
    /// Selects a slot (UI test).
    pub fn select(&mut self, day: usize, index: usize) {
        self.selected = Some((day, index));
    }
}

/// The library and theme, shared by everything that paints a slot.
struct Look<'a> {
    theme: Theme,
    items: &'a [Item],
    dir: &'a Path,
}

/// Applies an edit, keeping the version before it for Undo.
fn commit<R>(
    st: &mut TimelineState,
    tl: &mut Timeline,
    actions: &mut Vec<UiAction>,
    edit: impl FnOnce(&mut Timeline) -> R,
) -> R {
    let before = tl.clone();
    let r = edit(tl);
    if *tl != before {
        st.undo.push(before);
        if st.undo.len() > UNDO_STEPS {
            st.undo.remove(0);
        }
        changed(actions);
    }
    r
}

fn changed(actions: &mut Vec<UiAction>) {
    if !actions.contains(&UiAction::TimelineChanged) {
        actions.push(UiAction::TimelineChanged);
    }
}

fn undo(st: &mut TimelineState, tl: &mut Timeline, actions: &mut Vec<UiAction>) {
    if let Some(prev) = st.undo.pop() {
        *tl = prev;
        st.selected = None;
        st.drag = None;
        st.keep_day = None;
        changed(actions);
    }
}

/// Today as the editor counts days, and the minute of the day.
fn now() -> (usize, u16) {
    let n = jiff::ZonedDateTime::now();
    (
        timeline::weekday_index(n.weekday()),
        n.hour() as u16 * 60 + n.minute() as u16,
    )
}

fn day_label(tl: &Timeline, day: usize) -> &'static str {
    match tl.layout {
        Days::Single => "Every day",
        Days::Weekly => day_name(day),
    }
}

/// A slot's content in a few words.
fn describe(content: &Content, items: &[Item]) -> String {
    let name = |id: &str| items.iter().find(|i| i.id == id).map(|i| i.name.clone());
    match content {
        Content::Item { id } => name(id).unwrap_or_else(|| "Missing wallpaper".into()),
        Content::Pool { ids, tag, .. } => match (ids.len(), tag) {
            (0, Some(tag)) => format!("Pool: {tag}"),
            (0, None) => "Empty pool".into(),
            (n, None) => format!("Pool of {n}"),
            (n, Some(tag)) => format!("Pool of {n} and {tag}"),
        },
        Content::Random { archetypes, .. } if archetypes.is_empty() => "Random scene".into(),
        Content::Random { archetypes, .. } => {
            let names: Vec<&str> = archetypes
                .iter()
                .map(|a| {
                    view::LANDSCAPES
                        .iter()
                        .find(|(id, _, _)| id == a)
                        .map_or(a.as_str(), |(_, n, _)| *n)
                })
                .collect();
            format!("Random: {}", names.join(", "))
        }
    }
}

/// The library item whose thumbnail stands for a slot.
fn face<'a>(content: &'a Content, items: &'a [Item]) -> Option<&'a str> {
    let exists = |id: &str| items.iter().any(|i| i.id == id);
    match content {
        Content::Item { id } => exists(id).then_some(id.as_str()),
        Content::Pool { ids, tag, .. } => ids
            .iter()
            .find(|id| exists(id))
            .map(String::as_str)
            .or_else(|| {
                let tag = tag.as_ref()?;
                items
                    .iter()
                    .find(|i| i.tags.contains(tag))
                    .map(|i| i.id.as_str())
            }),
        Content::Random { .. } => None,
    }
}

/// The part of a 16:9 thumbnail that fills a block without stretching.
fn cover_uv(texture: egui::Vec2, target: egui::Vec2) -> Rect {
    let ta = texture.x / texture.y.max(1.0);
    let ra = target.x / target.y.max(1.0);
    if ra > ta {
        let h = ta / ra;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    } else {
        let w = ra / ta;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    }
}

pub fn show(
    ui: &mut Ui,
    theme: Theme,
    st: &mut TimelineState,
    model: &mut super::Model,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let look = Look {
        theme,
        items: model.library,
        dir: model.library_dir,
    };
    let status = model.status;
    let settings = &mut *model.settings;
    let tl = &mut *model.timeline;
    // Forget a selection or drag the timeline no longer has.
    let valid = |tl: &Timeline, d: usize, i: usize| d < tl.day_count() && i < tl.slots(d).len();
    if let Some((d, i)) = st.selected
        && !valid(tl, d, i)
    {
        st.selected = None;
    }
    if let Some((d, i, _)) = st.drag
        && !valid(tl, d, i)
    {
        st.drag = None;
    }
    keyboard(ui, st, tl, actions);

    egui::Panel::left("saved")
        .exact_size(SIDEBAR_W)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_sidebar))
                .inner_margin(egui::Margin::symmetric(space::S3 as i8, space::S4 as i8)),
        )
        .show(ui, |ui| sidebar(ui, &look, st));
    egui::Panel::right("slot")
        .exact_size(INSPECTOR_W)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_sidebar))
                .inner_margin(egui::Margin::symmetric(space::S4 as i8, space::S4 as i8)),
        )
        .show(ui, |ui| inspector(ui, &look, st, tl, actions));
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(c(t.bg))
                .inner_margin(egui::Margin::symmetric(space::S5 as i8, space::S5 as i8)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                header(ui, &look, st, tl, settings, status, actions);
                ui.add_space(space::S4);
                days(ui, &look, st, tl, actions);
            });
        });
}

/// Undo, Delete and slot selection by arrows, unless a text field has the keys.
fn keyboard(ui: &Ui, st: &mut TimelineState, tl: &mut Timeline, actions: &mut Vec<UiAction>) {
    let ctx = ui.ctx();
    if ctx.text_edit_focused() {
        return;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
        undo(st, tl, actions);
    }
    if st.drag.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        st.selected = None;
    }
    // Not while a slot edge is being dragged: the drag holds the slot's
    // index, and deleting under it would move the drag onto the next slot.
    if let Some((d, i)) = st.selected
        && st.drag.is_none()
        && ctx.input_mut(|inp| {
            inp.consume_key(Modifiers::NONE, Key::Delete)
                || inp.consume_key(Modifiers::NONE, Key::Backspace)
        })
    {
        commit(st, tl, actions, |tl| tl.remove(d, i));
        st.selected = None;
    }
    // Arrows move focus between widgets when one has it; otherwise they
    // move the selection.
    if ctx.memory(|m| m.focused().is_some()) {
        return;
    }
    let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
    if key(Key::ArrowRight) {
        st.selected = step_slot(tl, st.selected, 1);
    } else if key(Key::ArrowLeft) {
        st.selected = step_slot(tl, st.selected, -1);
    } else if key(Key::ArrowDown) {
        st.selected = step_day(tl, st.selected, 1);
    } else if key(Key::ArrowUp) {
        st.selected = step_day(tl, st.selected, -1);
    }
}

/// The next or previous slot, across days.
fn step_slot(tl: &Timeline, sel: Option<(usize, usize)>, step: i32) -> Option<(usize, usize)> {
    let all: Vec<(usize, usize)> = (0..tl.day_count())
        .flat_map(|d| (0..tl.slots(d).len()).map(move |i| (d, i)))
        .collect();
    if all.is_empty() {
        return None;
    }
    let next = match sel.and_then(|s| all.iter().position(|x| *x == s)) {
        None if step > 0 => 0,
        None => all.len() - 1,
        Some(p) => (p as i32 + step).rem_euclid(all.len() as i32) as usize,
    };
    Some(all[next])
}

/// The slot at the same time on the day above or below.
fn step_day(tl: &Timeline, sel: Option<(usize, usize)>, step: i32) -> Option<(usize, usize)> {
    let Some((d, i)) = sel else {
        return step_slot(tl, None, step);
    };
    let minute = tl.slots(d)[i].start;
    let n = tl.day_count() as i32;
    for k in 1..n {
        let nd = (d as i32 + step * k).rem_euclid(n) as usize;
        if !tl.slots(nd).is_empty() {
            return Some((nd, tl.slot_at(nd, minute).unwrap_or(0)));
        }
    }
    sel
}

/// Saved wallpapers, and a random scene, to drag onto a day: tiles that
/// lift and show a grip under the pointer, so they read as things to pick up.
fn sidebar(ui: &mut Ui, look: &Look, st: &mut TimelineState) {
    let t = colors(look.theme);
    let w = ui.available_width();
    let random = ui.dnd_drag_source(Id::unique("tl-drag-random"), Payload::Random, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
        st.random_chip = Some(rect);
        let hot = ui.rect_contains_pointer(rect);
        random_tile(ui.painter(), look.theme, rect, hot);
    });
    let _ = random.response.on_hover_text("Random scene");
    ui.add_space(space::S3);
    hairline(ui, t.hairline);
    ui.add_space(space::S3);
    egui::ScrollArea::vertical().show(ui, |ui| {
        if look.items.is_empty() {
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
            empty_library(ui.painter(), look.theme, rect);
            let _ = resp.on_hover_text("Wallpapers you save to the library show up here");
        }
        for item in look.items {
            ui.dnd_drag_source(
                Id::unique(("tl-drag", &item.id)),
                Payload::Item(item.id.clone()),
                |ui| {
                    ui.spacing_mut().item_spacing.y = space::S1;
                    let w = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
                    let hot = ui.rect_contains_pointer(rect);
                    match st.thumbs.get(ui.ctx(), look.dir, &item.id) {
                        Some(tex) => egui::Image::new(tex)
                            .corner_radius(radius::CONTROL)
                            .paint_at(ui, rect),
                        None => {
                            ui.painter()
                                .rect_filled(rect, radius::CONTROL, c(t.bg_raised));
                        }
                    }
                    if hot {
                        lifted(ui.painter(), look.theme, rect);
                    }
                    ui.label(RichText::new(&item.name).font(style::body_medium()));
                },
            );
            ui.add_space(space::S3);
        }
    });
}

/// The random scene's tile: a landscape in miniature, drawn in rows of dots
/// like the wallpapers, with a die on it.
fn random_tile(p: &egui::Painter, theme: Theme, r: Rect, hot: bool) {
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
fn lifted(p: &egui::Painter, theme: Theme, r: Rect) {
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
fn empty_library(p: &egui::Painter, theme: Theme, r: Rect) {
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

fn header(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    settings: &mut crate::settings::Settings,
    status: &view::Status,
    actions: &mut Vec<UiAction>,
) {
    let theme = look.theme;
    let t = colors(theme);
    let (today, _) = now();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Timeline").font(style::title()));
        ui.add_space(space::S3);
        // Whether Rotation follows it.
        let mut on = settings.rotation.mode == RotationMode::Timeline;
        let switch = view::toggle(ui, theme, &mut on, "Use the timeline");
        if switch.changed() {
            settings.rotation.mode = if on {
                RotationMode::Timeline
            } else {
                RotationMode::Generate
            };
            actions.push(UiAction::SettingsChanged);
        }
        let _ = switch.on_hover_text(if on {
            "On: the wallpaper follows this timeline"
        } else {
            "Off: new scenes on the Generate schedule"
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut layout = tl.layout;
            if segmented(
                ui,
                theme,
                &mut layout,
                &[
                    (Days::Single, "Daily", true),
                    (Days::Weekly, "Weekly", true),
                ],
            ) {
                match layout {
                    Days::Weekly => {
                        commit(st, tl, actions, Timeline::make_weekly);
                        st.keep_day = None;
                    }
                    Days::Single => st.keep_day = Some(today),
                }
            }
            ui.add_space(space::S3);
            let undo_tip = if cfg!(target_os = "macos") {
                "Undo  \u{2318}Z"
            } else {
                "Undo  Ctrl+Z"
            };
            if icon_btn(ui, theme, Glyph::Undo, undo_tip, !st.undo.is_empty(), false).clicked() {
                undo(st, tl, actions);
            }
            // Something a slot needs is gone: a mark, the details on hover.
            if !status.timeline_problems.is_empty() {
                ui.add_space(space::S2);
                let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                glyph(ui.painter(), Glyph::Warning, r.shrink(3.0), c(t.danger));
                let _ = resp.on_hover_text(status.timeline_problems.join("\n"));
            }
        });
    });
    ui.add_space(space::S2);
    hairline(ui, t.hairline);
    ui.add_space(space::S3);

    if let Some(mut keep) = st.keep_day {
        egui::Frame::new()
            .fill(c(t.bg_raised))
            .stroke(Stroke::new(size::HAIRLINE, c(t.hairline)))
            .corner_radius(radius::PANEL)
            .inner_margin(egui::Margin::same(space::S3 as i8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Keep which day?").font(style::body_strong()));
                    ui.add_space(space::S2);
                    for d in 0..7 {
                        if day_chip(ui, theme, short_day(d), keep == d, false, true).clicked() {
                            keep = d;
                        }
                    }
                });
                st.keep_day = Some(keep);
                ui.add_space(space::S2);
                ui.horizontal(|ui| {
                    if primary_button(ui, theme, "Keep").clicked() {
                        commit(st, tl, actions, |tl| tl.make_single(keep));
                        st.keep_day = None;
                        st.selected = None;
                    }
                    if ui.button("Cancel").clicked() {
                        st.keep_day = None;
                    }
                });
            });
        ui.add_space(space::S3);
    }
}

/// A day as three letters.
fn short_day(d: usize) -> &'static str {
    let name = day_name(d);
    &name[..3.min(name.len())]
}

/// A round chip for a day: on, off, or marked as about to be replaced.
fn day_chip(
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
enum Glyph {
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
fn glyph(p: &egui::Painter, g: Glyph, r: Rect, ink: egui::Color32) {
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
fn icon_btn(
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
fn hatch(p: &egui::Painter, r: Rect, ink: egui::Color32) {
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

/// The hour scale and one row per day.
fn days(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(look.theme);
    let n = tl.day_count();
    let (today, minute) = now();
    let (row_h, bar_h) = if n == 1 { (88.0, 64.0) } else { (60.0, 44.0) };
    let w = ui.available_width();
    let (scale, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    let bars = Rect::from_min_max(
        pos2(scale.left() + LABEL_W, scale.top()),
        pos2(scale.right(), scale.bottom() + row_h * n as f32),
    );
    // Hour lines every three hours, under the bars.
    for h in (0..=24).step_by(3) {
        let x = bars.left() + bars.width() * h as f32 / 24.0;
        let align = match h {
            0 => Align2::LEFT_BOTTOM,
            24 => Align2::RIGHT_BOTTOM,
            _ => Align2::CENTER_BOTTOM,
        };
        ui.painter().text(
            pos2(x, scale.bottom() - 3.0),
            align,
            format!("{h:02}"),
            style::mono(),
            c(t.text_secondary),
        );
        ui.painter().line_segment(
            [pos2(x, scale.bottom()), pos2(x, bars.bottom())],
            Stroke::new(size::HAIRLINE, c(t.hairline)),
        );
    }
    st.bars.clear();
    for day in 0..n {
        let (row, _) = ui.allocate_exact_size(vec2(w, row_h), Sense::hover());
        let label = Rect::from_min_max(
            row.min,
            pos2(row.left() + LABEL_W - space::S3, row.bottom()),
        );
        let bar = Rect::from_min_max(
            pos2(row.left() + LABEL_W, row.center().y - bar_h / 2.0),
            pos2(row.right(), row.center().y + bar_h / 2.0),
        );
        let is_today = n == 1 || day == today;
        let hot = ui.rect_contains_pointer(row);
        day_actions(ui, look, st, tl, day, label, is_today, hot, actions);
        st.bars.push(bar);
        day_bar(
            ui,
            look,
            st,
            tl,
            day,
            bar,
            is_today.then_some(minute),
            actions,
        );
    }
}

/// The day's name (a dot for today) and its buttons as marks: add a slot,
/// copy the day, clear it. Quiet until the row is under the pointer.
#[allow(clippy::too_many_arguments)]
fn day_actions(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    day: usize,
    rect: Rect,
    is_today: bool,
    hot: bool,
    actions: &mut Vec<UiAction>,
) {
    let theme = look.theme;
    let t = colors(theme);
    let mut ui = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min))
            .id_salt(("tl-day", day)),
    );
    ui.spacing_mut().item_spacing = vec2(space::S1, 2.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(day_label(tl, day)).font(style::body_strong()));
        if is_today && tl.layout == Days::Weekly {
            let (r, resp) = ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.0, c(t.accent));
            let _ = resp.on_hover_text("Today");
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let faint = !hot;
        if icon_btn(ui, theme, Glyph::Plus, "Add a slot", true, faint).clicked()
            && let Some(i) = commit(st, tl, actions, |tl| tl.add_slot(day))
        {
            st.selected = Some((day, i));
        }
        if tl.layout == Days::Weekly {
            let copy = icon_btn(ui, theme, Glyph::Copy, "Copy to other days", true, faint);
            if copy.clicked() {
                st.copy_to = [false; 7];
            }
            egui::Popup::menu(&copy)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| copy_picker(ui, theme, st, tl, day, actions));
        }
        let any = !tl.slots(day).is_empty();
        if icon_btn(ui, theme, Glyph::Clear, "Clear the day", any, faint).clicked() {
            commit(st, tl, actions, |tl| tl.clear_day(day));
            if st.selected.is_some_and(|(d, _)| d == day) {
                st.selected = None;
            }
        }
    });
}

/// Copy a day over others: the days as chips (one about to lose its slots
/// is ringed in red), quick picks, and the button.
fn copy_picker(
    ui: &mut Ui,
    theme: Theme,
    st: &mut TimelineState,
    tl: &mut Timeline,
    day: usize,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        for d in 0..7 {
            let on = d == day || st.copy_to[d];
            let replaced = d != day && st.copy_to[d] && !tl.slots(d).is_empty();
            if day_chip(ui, theme, short_day(d), on, replaced, d != day).clicked() {
                st.copy_to[d] = !st.copy_to[d];
            }
        }
    });
    ui.add_space(space::S2);
    ui.horizontal(|ui| {
        if ui.button("All").clicked() {
            st.copy_to = [true; 7];
        }
        if ui.button("Weekdays").clicked() {
            st.copy_to = [true, true, true, true, true, false, false];
        }
        if ui.button("Weekend").clicked() {
            st.copy_to = [false, false, false, false, false, true, true];
        }
    });
    ui.add_space(space::S2);
    let targets: Vec<usize> = (0..7).filter(|d| *d != day && st.copy_to[*d]).collect();
    let replacing = targets.iter().any(|d| !tl.slots(*d).is_empty());
    let label = if replacing { "Replace" } else { "Copy" };
    ui.add_enabled_ui(!targets.is_empty(), |ui| {
        if primary_button(ui, theme, label).clicked() {
            commit(st, tl, actions, |tl| tl.copy_day(day, &targets));
            st.copy_to = [false; 7];
            ui.close();
        }
    });
}

/// The slot whose left edge is under `x`, if any.
fn edge_at(starts: &[u16], bar: Rect, x: f32) -> Option<usize> {
    starts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let sx = bar.left() + bar.width() * f32::from(*s) / f32::from(DAY_MINUTES);
            (i, (sx - x).abs())
        })
        .filter(|(_, d)| *d <= EDGE_GRAB)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// One day's 24-hour bar: painting and every pointer interaction.
#[allow(clippy::too_many_arguments)]
fn day_bar(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    day: usize,
    bar: Rect,
    now_minute: Option<u16>,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(look.theme);
    let resp = ui.interact(bar, Id::unique(("tl-bar", day)), Sense::click_and_drag());
    let x_of = |m: u16| bar.left() + bar.width() * f32::from(m) / f32::from(DAY_MINUTES);
    let minute_at = |x: f32| (x - bar.left()) / bar.width() * f32::from(DAY_MINUTES);
    let fine = ui.input(|i| i.modifiers.shift);
    let pointer = ui.ctx().pointer_latest_pos();

    // The starts as shown, with a drag in progress.
    let mut starts: Vec<u16> = tl.slots(day).iter().map(|s| s.start).collect();
    if let Some((d, i, m)) = st.drag
        && d == day
    {
        starts[i] = m;
    }
    if resp.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && let Some(i) = edge_at(&starts, bar, origin.x)
    {
        st.drag = Some((day, i, starts[i]));
        st.selected = Some((day, i));
    }
    let mut dragging = None;
    if let Some((d, i, _)) = st.drag
        && d == day
    {
        if let Some(p) = resp.interact_pointer_pos() {
            let lo = if i == 0 { 0 } else { starts[i - 1] + 1 };
            let hi = starts.get(i + 1).map_or(DAY_MINUTES - 1, |s| s - 1);
            starts[i] = snap(minute_at(p.x), fine).clamp(lo, hi.max(lo));
            st.drag = Some((d, i, starts[i]));
        }
        if resp.drag_stopped() {
            st.drag = None;
            let m = starts[i];
            commit(st, tl, actions, |tl| tl.move_start(d, i, m));
            st.selected = Some((d, i));
        } else {
            dragging = Some(i);
        }
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    } else if let Some(h) = resp.hover_pos()
        && edge_at(&starts, bar, h.x).is_some()
    {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }

    // A click on a slot opens it; on empty time (the Generate schedule's)
    // it adds a slot there, as the ghost under the pointer shows.
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let m = (minute_at(p.x).max(0.0) as u16).min(DAY_MINUTES - 1);
        let empty = tl.slots(day).is_empty();
        st.selected = match (tl.slot_at(day, m), tl.carried_into(day)) {
            (Some(i), _) => Some((day, i)),
            (None, Carried::Slot(d, i)) if !empty => Some((d, i)),
            _ => {
                let at = snap(minute_at(p.x), fine).min(DAY_MINUTES - 1);
                let i = commit(st, tl, actions, |tl| tl.insert(day, Slot::random(at)));
                Some((day, i))
            }
        };
    }
    if resp.double_clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let m = snap(minute_at(p.x), fine);
        if let Some(i) = commit(st, tl, actions, |tl| tl.split(day, m)) {
            st.selected = Some((day, i));
        }
    }
    let dropping = resp.dnd_hover_payload::<Payload>().is_some();
    if let Some(payload) = resp.dnd_release_payload::<Payload>()
        && let Some(p) = pointer
    {
        let m = snap(minute_at(p.x), fine);
        let slot = match &*payload {
            Payload::Item(id) => Slot {
                content: Content::Item { id: id.clone() },
                ..Slot::random(m)
            },
            Payload::Random => Slot::random(m),
        };
        let i = commit(st, tl, actions, |tl| tl.insert(day, slot));
        st.selected = Some((day, i));
    }

    // Painting, from the timeline as it is after this frame's edits.
    let mut slots: Vec<Slot> = tl.slots(day).to_vec();
    if let Some((d, i, m)) = st.drag
        && d == day
        && i < slots.len()
    {
        slots[i].start = m;
    }
    let seg = |a: u16, b: u16| {
        Rect::from_min_max(
            pos2(x_of(a) + 1.0, bar.top()),
            pos2(x_of(b) - 1.0, bar.bottom()),
        )
    };
    let painter = ui.painter().clone();
    let generate = |r: Rect| {
        painter.rect(
            r,
            radius::CONTROL,
            c(t.bg_sidebar),
            Stroke::new(size::HAIRLINE, c(t.hairline)),
            StrokeKind::Inside,
        );
        hatch(&painter, r, alpha(t.text_secondary, 0.14));
    };
    if slots.is_empty() {
        generate(bar);
    } else {
        if slots[0].start > 0 {
            let r = seg(0, slots[0].start);
            match tl.carried_into(day) {
                Carried::Slot(d, i) => {
                    let content = tl.slots(d)[i].content.clone();
                    block(ui, look, st, r, &content, None);
                    if st.selected == Some((d, i)) {
                        selected_ring(&painter, look.theme, r);
                    }
                }
                Carried::Generate => generate(r),
            }
        }
        for (i, s) in slots.iter().enumerate() {
            let end = slots.get(i + 1).map_or(DAY_MINUTES, |n| n.start);
            let r = seg(s.start, end);
            block(ui, look, st, r, &s.content, Some(s.start));
            if st.selected == Some((day, i)) {
                selected_ring(&painter, look.theme, r);
            }
        }
    }
    if resp.hovered() || dragging.is_some() {
        let near = resp.hover_pos().and_then(|h| edge_at(&starts, bar, h.x));
        for (k, s) in slots.iter().enumerate() {
            let x = x_of(s.start);
            let grip = Rect::from_center_size(
                pos2(x, bar.center().y),
                vec2(4.0, (bar.height() * 0.45).min(18.0)),
            );
            let strong = near == Some(k) || dragging == Some(k);
            painter.rect_filled(grip.expand(1.5), 3.0, c(t.bg).gamma_multiply(0.8));
            painter.rect_filled(grip, 2.0, alpha(t.text, if strong { 1.0 } else { 0.55 }));
        }
    }
    if let Some(m) = now_minute {
        let x = x_of(m);
        painter.line_segment(
            [pos2(x, bar.top() - 4.0), pos2(x, bar.bottom() + 4.0)],
            Stroke::new(2.0, c(t.accent)),
        );
        painter.circle_filled(pos2(x, bar.top() - 4.0), 3.0, c(t.accent));
    }
    // Where a drop or a dragged edge lands.
    let marker = match (dragging, dropping, pointer) {
        (Some(i), _, _) => Some(slots[i].start),
        (None, true, Some(p)) => Some(snap(minute_at(p.x), fine)),
        _ => None,
    };
    if let Some(m) = marker {
        let x = x_of(m);
        painter.line_segment(
            [pos2(x, bar.top() - 2.0), pos2(x, bar.bottom() + 2.0)],
            Stroke::new(2.0, c(t.text)),
        );
        pill(
            &painter,
            look.theme,
            pos2(x + 4.0, bar.top() + 3.0),
            &format_time(m),
        );
    } else if let Some(h) = resp.hover_pos()
        && edge_at(&starts, bar, h.x).is_none()
        && tl
            .slot_at(day, (minute_at(h.x).max(0.0) as u16).min(DAY_MINUTES - 1))
            .is_none()
        && (tl.slots(day).is_empty() || tl.carried_into(day) == Carried::Generate)
    {
        // Empty time: the slot a click would add, from here to the next.
        let from = snap(minute_at(h.x), fine).min(DAY_MINUTES - 1);
        let to = tl
            .slots(day)
            .iter()
            .map(|s| s.start)
            .find(|s| *s > from)
            .unwrap_or(DAY_MINUTES);
        let g = seg(from, to);
        painter.rect_filled(g, radius::CONTROL, alpha(t.accent, 0.12));
        painter.rect_stroke(
            g,
            radius::CONTROL,
            Stroke::new(1.4, alpha(t.accent, 0.85)),
            StrokeKind::Inside,
        );
        let plus = Rect::from_center_size(pos2(g.left() + 14.0, g.center().y), vec2(14.0, 14.0));
        if g.width() > 24.0 {
            glyph(&painter, Glyph::Plus, plus, c(t.accent_text));
        }
        if g.width() > 80.0 {
            pill(
                &painter,
                look.theme,
                pos2(g.left() + 26.0, g.center().y - 9.0),
                &format_time(from),
            );
        }
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    } else if let Some(h) = resp.hover_pos() {
        // Preview: what would be on screen at the time under the pointer.
        let m = (minute_at(h.x).max(0.0) as u16).min(DAY_MINUTES - 1);
        painter.line_segment(
            [pos2(h.x, bar.top()), pos2(h.x, bar.bottom())],
            Stroke::new(size::HAIRLINE, c(t.text)),
        );
        let content = match tl.slot_at(day, m) {
            Some(i) => Some(tl.slots(day)[i].content.clone()),
            None if tl.slots(day).is_empty() => None,
            None => match tl.carried_into(day) {
                Carried::Slot(d, i) => Some(tl.slots(d)[i].content.clone()),
                Carried::Generate => None,
            },
        };
        let when = format!("{} {}", day_label(tl, day), format_time(m));
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(RichText::new(when).font(style::mono()));
            match &content {
                Some(content) => {
                    ui.label(describe(content, look.items));
                    if let Some(id) = face(content, look.items)
                        && let Some(tex) = st.thumbs.get(ui.ctx(), look.dir, id)
                    {
                        ui.add(
                            egui::Image::new(tex)
                                .fit_to_exact_size(vec2(192.0, 108.0))
                                .corner_radius(radius::CONTROL),
                        );
                    }
                }
                None => {
                    ui.label("New scenes on the Generate schedule");
                }
            }
        });
    }
}

fn selected_ring(painter: &egui::Painter, theme: Theme, r: Rect) {
    painter.rect_stroke(
        r,
        radius::CONTROL,
        Stroke::new(size::FOCUS_RING, c(colors(theme).accent)),
        StrokeKind::Inside,
    );
}

/// A small label on a scrim, readable over any wallpaper.
fn pill(painter: &egui::Painter, theme: Theme, at: egui::Pos2, label: &str) -> Rect {
    let t = colors(theme);
    let galley = painter.layout_no_wrap(label.to_string(), style::mono(), c(t.text));
    let r = Rect::from_min_size(at, galley.size() + vec2(8.0, 2.0));
    painter.rect_filled(r, 4.0, c(t.bg).gamma_multiply(0.88));
    painter.galley(r.min + vec2(4.0, 1.0), galley, c(t.text));
    r
}

/// One slot block: its wallpaper (or plain for random scenes), the start
/// and the name when they fit. `start` is `None` for the part carried over
/// from the day before, which is drawn faded.
fn block(
    ui: &Ui,
    look: &Look,
    st: &mut TimelineState,
    r: Rect,
    content: &Content,
    start: Option<u16>,
) {
    let t = colors(look.theme);
    if r.width() < 1.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(r);
    let tex = face(content, look.items)
        .and_then(|id| st.thumbs.get(ui.ctx(), look.dir, id))
        .cloned();
    match &tex {
        Some(tex) => {
            egui::Image::new(tex)
                .uv(cover_uv(tex.size_vec2(), r.size()))
                .corner_radius(radius::CONTROL)
                .paint_at(ui, r);
        }
        None => {
            painter.rect_filled(r, radius::CONTROL, c(t.bg_raised));
        }
    }
    painter.rect_stroke(
        r,
        radius::CONTROL,
        Stroke::new(size::HAIRLINE, c(t.border_control)),
        StrokeKind::Inside,
    );
    let missing = matches!(content, Content::Item { .. }) && tex.is_none();
    if missing && r.width() >= 28.0 {
        let w = Rect::from_min_size(pos2(r.right() - 24.0, r.top() + 4.0), vec2(20.0, 20.0));
        painter.rect_filled(w, 5.0, c(t.bg).gamma_multiply(0.85));
        glyph(&painter, Glyph::Warning, w.shrink(3.0), c(t.danger));
    }
    let Some(start) = start else {
        painter.rect_filled(r, radius::CONTROL, c(t.bg).gamma_multiply(0.55));
        return;
    };
    if r.width() < 40.0 {
        return;
    }
    let time = pill(
        &painter,
        look.theme,
        r.min + vec2(3.0, 3.0),
        &format_time(start),
    );
    if r.width() >= 72.0 && r.height() >= 40.0 {
        let name = describe(content, look.items);
        let font = FontId::new(text::SMALL.size, FontFamily::Proportional);
        let color = if missing { c(t.danger) } else { c(t.text) };
        let galley = painter.layout_no_wrap(name, font, color);
        let nr = Rect::from_min_size(
            pos2(time.left(), time.bottom() + 2.0),
            galley.size() + vec2(8.0, 2.0),
        );
        painter.rect_filled(nr, 4.0, c(t.bg).gamma_multiply(0.88));
        painter.galley(nr.min + vec2(4.0, 1.0), galley, color);
    }
}

/// When a slot gives way: the next slot, or for a day's last slot, the
/// next day's first (or midnight, when the next day is empty).
fn until(tl: &Timeline, day: usize, index: usize) -> String {
    let end = tl.end_of(day, index);
    if end < DAY_MINUTES {
        return format_time(end);
    }
    let next = (day + 1) % tl.day_count();
    match (tl.slots(next).first(), tl.layout) {
        (None, _) => "00:00".into(),
        (Some(s), _) if s.start == 0 => "00:00".into(),
        (Some(s), Days::Single) => format!("{} next day", format_time(s.start)),
        (Some(s), Days::Weekly) => format!("{} {}", format_time(s.start), day_name(next)),
    }
}

/// What the editor does, shown rather than told (the way macOS's trackpad
/// settings show each gesture): a pointer drops a wallpaper on a day, moves
/// a slot's start, then opens a slot, over and over; and the keys, as keys.
fn demo(ui: &mut Ui, look: &Look, st: &mut TimelineState) {
    let theme = look.theme;
    let t = colors(theme);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 188.0), Sense::hover());
    let now = ui.input(|i| i.time) as f32;
    const ACT: f32 = 3.4;
    let act = ((now / ACT) as u32 % 3) as usize;
    let u = (now % ACT) / ACT;
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let span = |a: f32, b: f32| ease((u - a) / (b - a));
    let thumb = look
        .items
        .first()
        .and_then(|it| st.thumbs.get(ui.ctx(), look.dir, &it.id))
        .cloned();
    let p = ui.painter().clone();
    p.rect_filled(r, radius::PANEL, alpha(t.text, 0.03));
    p.rect_stroke(
        r,
        radius::PANEL,
        Stroke::new(size::HAIRLINE, c(t.hairline)),
        StrokeKind::Inside,
    );
    let bar = Rect::from_min_size(pos2(r.left() + 14.0, r.top() + 108.0), vec2(w - 28.0, 36.0));
    let x_at = |f: f32| bar.left() + bar.width() * f;
    let part = |a: f32, b: f32| {
        Rect::from_min_max(
            pos2(x_at(a) + 1.0, bar.top()),
            pos2(x_at(b) - 1.0, bar.bottom()),
        )
    };
    let tile = Rect::from_min_size(pos2(r.left() + 14.0, r.top() + 16.0), vec2(80.0, 45.0));
    let picture = |p: &egui::Painter, rr: Rect, fade: f32| {
        match &thumb {
            Some(tex) => {
                p.image(
                    tex.id(),
                    rr,
                    cover_uv(tex.size_vec2(), rr.size()),
                    egui::Color32::WHITE.gamma_multiply(fade),
                );
            }
            None => random_tile(p, theme, rr, false),
        }
        p.rect_stroke(
            rr,
            radius::CONTROL,
            Stroke::new(size::HAIRLINE, alpha(t.border_control, fade)),
            StrokeKind::Inside,
        );
    };
    let plain = |p: &egui::Painter, rr: Rect| {
        p.rect_filled(rr, radius::CONTROL, c(t.bg_raised));
        p.rect_stroke(
            rr,
            radius::CONTROL,
            Stroke::new(size::HAIRLINE, c(t.border_control)),
            StrokeKind::Inside,
        );
    };
    let empty = |p: &egui::Painter, rr: Rect| {
        p.rect_filled(rr, radius::CONTROL, c(t.bg_sidebar));
        hatch(p, rr, alpha(t.text_secondary, 0.14));
    };
    let clock = |f: f32| format_time(((f * 24.0 * 4.0).round() as u16 * 15).min(DAY_MINUTES - 15));
    match act {
        // Drag a wallpaper onto the day.
        0 => {
            picture(&p, tile, 1.0);
            let drop_at = 0.55;
            let target = pos2(x_at(drop_at) + 18.0, bar.center().y);
            let from = tile.center();
            let moving = span(0.12, 0.62);
            let at = from + (target - from) * moving;
            let dropped = u > 0.66;
            if dropped {
                empty(&p, part(0.0, drop_at));
                let k = span(0.66, 0.8);
                picture(&p, part(drop_at, 1.0), k);
                pill(
                    &p,
                    theme,
                    pos2(x_at(drop_at) + 4.0, bar.top() + 3.0),
                    &clock(drop_at),
                );
            } else {
                empty(&p, bar);
                if u > 0.12 {
                    let ghost = Rect::from_center_size(at - vec2(18.0, 0.0), tile.size() * 0.8);
                    picture(&p, ghost, 0.75);
                }
                if moving > 0.85 {
                    let x = x_at(drop_at);
                    p.line_segment(
                        [pos2(x, bar.top() - 2.0), pos2(x, bar.bottom() + 2.0)],
                        Stroke::new(2.0, c(t.text)),
                    );
                }
            }
            pointer(&p, at, u > 0.1 && u < 0.66);
        }
        // Move a slot's start by its edge.
        1 => {
            let edge = 0.38 + 0.24 * span(0.32, 0.78);
            plain(&p, part(0.0, edge));
            picture(&p, part(edge, 1.0), 1.0);
            let grip = Rect::from_center_size(pos2(x_at(edge), bar.center().y), vec2(4.0, 16.0));
            p.rect_filled(grip.expand(1.5), 3.0, c(t.bg).gamma_multiply(0.8));
            p.rect_filled(grip, 2.0, c(t.text));
            let rest = pos2(r.right() - 36.0, r.top() + 36.0);
            let grab = pos2(x_at(0.38), bar.center().y);
            let at = if u < 0.3 {
                rest + (grab - rest) * span(0.0, 0.28)
            } else {
                pos2(x_at(edge), bar.center().y)
            };
            if u >= 0.3 {
                pill(
                    &p,
                    theme,
                    pos2(x_at(edge) + 6.0, bar.top() - 20.0),
                    &clock(edge),
                );
                resize_pointer(&p, at);
            } else {
                pointer(&p, at, false);
            }
        }
        // Click a slot to change what it shows.
        _ => {
            plain(&p, part(0.0, 0.45));
            let chosen = part(0.45, 1.0);
            picture(&p, chosen, 1.0);
            let rest = pos2(r.left() + 40.0, r.top() + 30.0);
            let target = chosen.center();
            let at = rest + (target - rest) * span(0.0, 0.3);
            let clicked = u > 0.34;
            if clicked {
                p.rect_stroke(
                    chosen,
                    radius::CONTROL,
                    Stroke::new(size::FOCUS_RING, c(t.accent)),
                    StrokeKind::Inside,
                );
                let ring = span(0.34, 0.6);
                p.circle_stroke(
                    target,
                    6.0 + 22.0 * ring,
                    Stroke::new(1.5, alpha(t.accent, 1.0 - ring)),
                );
                // The slot's choices slide in beside it.
                let k = span(0.42, 0.62);
                let card = Rect::from_min_size(
                    pos2(r.right() - 118.0 + (1.0 - k) * 24.0, r.top() + 14.0),
                    vec2(104.0, 74.0),
                );
                p.rect_filled(card, radius::CONTROL, alpha(t.bg_raised, k));
                p.rect_stroke(
                    card,
                    radius::CONTROL,
                    Stroke::new(size::HAIRLINE, alpha(t.border_control, k)),
                    StrokeKind::Inside,
                );
                for (n, len) in [0.7_f32, 0.5, 0.85].iter().enumerate() {
                    let y = card.top() + 16.0 + n as f32 * 18.0;
                    p.rect_filled(
                        Rect::from_min_size(
                            pos2(card.left() + 10.0, y),
                            vec2((card.width() - 20.0) * len, 6.0),
                        ),
                        3.0,
                        alpha(t.text_secondary, 0.5 * k),
                    );
                }
            }
            pointer(&p, at, clicked && u < 0.4);
        }
    }
    for k in 0..3 {
        let cx = r.center().x + (k as f32 - 1.0) * 12.0;
        let ink = if k == act {
            c(t.accent_deep)
        } else {
            alpha(t.text_secondary, 0.35)
        };
        p.circle_filled(pos2(cx, r.bottom() - 16.0), 3.0, ink);
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
    ui.add_space(space::S5);
    let undo = if cfg!(target_os = "macos") {
        "Cmd Z"
    } else {
        "Ctrl Z"
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S4;
        keycap(ui, theme, None, "Pick");
        keycap(ui, theme, Some("Del"), "Delete");
        keycap(ui, theme, Some(undo), "Undo");
        keycap(ui, theme, Some("Esc"), "Let go");
    });
}

/// A key drawn as a key, its job under it. `None`: the arrow keys.
fn keycap(ui: &mut Ui, theme: Theme, label: Option<&str>, job: &str) {
    let t = colors(theme);
    let font = FontId::new(text::SMALL.size, FontFamily::Monospace);
    let cap_w = match label {
        None => 54.0,
        Some(l) => (l.chars().count() as f32 * 7.4 + 16.0).max(30.0),
    };
    let (r, _) = ui.allocate_exact_size(vec2(cap_w.max(44.0), 46.0), Sense::hover());
    let p = ui.painter();
    let cap = Rect::from_min_size(pos2(r.center().x - cap_w / 2.0, r.top()), vec2(cap_w, 26.0));
    p.rect_filled(
        cap.translate(vec2(0.0, 2.0)),
        6.0,
        alpha(t.border_control, 0.5),
    );
    p.rect_filled(cap, 6.0, c(t.bg_raised));
    p.rect_stroke(
        cap,
        6.0,
        Stroke::new(size::HAIRLINE, c(t.border_control)),
        StrokeKind::Inside,
    );
    match label {
        Some(l) => {
            p.text(cap.center(), Align2::CENTER_CENTER, l, font, c(t.text));
        }
        None => {
            let half = |dx: f32, g: Glyph| {
                glyph(
                    p,
                    g,
                    Rect::from_center_size(cap.center() + vec2(dx, 0.0), vec2(12.0, 12.0)),
                    c(t.text),
                )
            };
            half(-10.0, Glyph::Earlier);
            half(10.0, Glyph::Later);
        }
    }
    p.text(
        pos2(r.center().x, cap.bottom() + 12.0),
        Align2::CENTER_CENTER,
        job,
        FontId::new(text::SMALL.size - 1.0, FontFamily::Proportional),
        c(t.text_secondary),
    );
}

/// The arrow pointer, white with a dark edge; smaller while pressed.
fn pointer(p: &egui::Painter, at: egui::Pos2, pressed: bool) {
    let s = if pressed { 0.88 } else { 1.0 };
    let pt = |x: f32, y: f32| at + vec2(x * s, y * s);
    let head = vec![pt(0.0, 0.0), pt(0.0, 15.0), pt(11.0, 11.0)];
    let tail = vec![pt(4.0, 11.6), pt(6.6, 17.4), pt(9.0, 16.4), pt(6.4, 10.8)];
    let white = egui::Color32::WHITE;
    p.add(egui::Shape::convex_polygon(head, white, Stroke::NONE));
    p.add(egui::Shape::convex_polygon(tail, white, Stroke::NONE));
    p.add(egui::Shape::closed_line(
        vec![
            pt(0.0, 0.0),
            pt(0.0, 15.0),
            pt(4.0, 11.6),
            pt(6.6, 17.4),
            pt(9.0, 16.4),
            pt(6.4, 10.8),
            pt(11.0, 11.0),
        ],
        Stroke::new(1.2, egui::Color32::from_gray(20)),
    ));
}

/// The pointer as it is over a slot's edge: arrows both ways.
fn resize_pointer(p: &egui::Painter, at: egui::Pos2) {
    for (stroke, ink) in [
        (4.0, egui::Color32::from_gray(20)),
        (1.8, egui::Color32::WHITE),
    ] {
        let st = Stroke::new(stroke, ink);
        p.line_segment([at - vec2(9.0, 0.0), at + vec2(9.0, 0.0)], st);
        p.add(egui::Shape::line(
            vec![
                at + vec2(-5.0, -4.0),
                at - vec2(9.0, 0.0),
                at + vec2(-5.0, 4.0),
            ],
            st,
        ));
        p.add(egui::Shape::line(
            vec![
                at + vec2(5.0, -4.0),
                at + vec2(9.0, 0.0),
                at + vec2(5.0, 4.0),
            ],
            st,
        ));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Saved,
    Pool,
    Random,
}

/// The selected slot's choices; help when nothing is selected.
fn inspector(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    actions: &mut Vec<UiAction>,
) {
    let theme = look.theme;
    let t = colors(theme);
    let Some((d, i)) = st.selected else {
        demo(ui, look, st);
        return;
    };
    let slot = tl.slots(d)[i].clone();
    ui.label(RichText::new(day_label(tl, d)).font(style::title()));
    ui.label(
        RichText::new(format!(
            "{} to {}",
            format_time(slot.start),
            until(tl, d, i)
        ))
        .font(style::mono())
        .color(c(t.text_secondary)),
    );
    ui.add_space(space::S2);
    hairline(ui, t.hairline);
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Start: typed, or nudged by a quarter of an hour.
        if st.start_for != Some((d, i, slot.start)) {
            st.start_text = format_time(slot.start);
            st.start_for = Some((d, i, slot.start));
        }
        group(ui, "Starts at");
        let mut moved = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut st.start_text)
                    // "00:00" in full, with the field's own margins.
                    .desired_width(76.0)
                    .font(FontId::new(text::BODY.size, FontFamily::Monospace)),
            );
            if field.lost_focus() {
                match timeline::parse_time(&st.start_text) {
                    Some(m) if m != slot.start => {
                        if let Some(j) = commit(st, tl, actions, |tl| tl.set_start(d, i, m)) {
                            st.selected = Some((d, j));
                            moved = true;
                        }
                    }
                    _ => {}
                }
                st.start_for = None;
            }
            if icon_btn(ui, theme, Glyph::Earlier, "15 minutes earlier", true, false).clicked() {
                commit(st, tl, actions, |tl| {
                    tl.move_start(d, i, slot.start.saturating_sub(15))
                });
                moved = true;
            }
            if icon_btn(ui, theme, Glyph::Later, "15 minutes later", true, false).clicked() {
                commit(st, tl, actions, |tl| tl.move_start(d, i, slot.start + 15));
                moved = true;
            }
        });
        if moved {
            return;
        }

        let mut s = slot.clone();
        group(ui, "Shows");
        let mut kind = match &s.content {
            Content::Item { .. } => Kind::Saved,
            Content::Pool { .. } => Kind::Pool,
            Content::Random { .. } => Kind::Random,
        };
        let have = !look.items.is_empty();
        let switched = ui
            .horizontal(|ui| {
                segmented(
                    ui,
                    theme,
                    &mut kind,
                    &[
                        (Kind::Saved, "Saved", have),
                        (Kind::Pool, "Pool", have),
                        (Kind::Random, "Random", true),
                    ],
                )
            })
            .inner;
        if switched {
            let current = match &s.content {
                Content::Item { id } => Some(id.clone()),
                Content::Pool { ids, .. } => ids.first().cloned(),
                Content::Random { .. } => None,
            };
            s.content = match kind {
                Kind::Saved => Content::Item {
                    id: current.unwrap_or_else(|| look.items[0].id.clone()),
                },
                Kind::Pool => Content::Pool {
                    ids: current.into_iter().collect(),
                    tag: None,
                    order: Order::Shuffle,
                },
                Kind::Random => Slot::random(0).content,
            };
        }
        ui.add_space(space::S2);
        match &mut s.content {
            Content::Item { id } => {
                let name = describe(&Content::Item { id: id.clone() }, look.items);
                egui::ComboBox::from_id_salt(("tl-item", d, i))
                    .width(ui.available_width())
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for item in look.items {
                            ui.selectable_value(id, item.id.clone(), &item.name);
                        }
                    });
                ui.add_space(space::S2);
                let w = ui.available_width();
                if let Some(tex) = st.thumbs.get(ui.ctx(), look.dir, id) {
                    ui.add(
                        egui::Image::new(tex)
                            .fit_to_exact_size(vec2(w, w * 9.0 / 16.0))
                            .corner_radius(radius::CONTROL),
                    );
                }
            }
            Content::Pool { ids, tag, order } => {
                for item in look.items {
                    let mut on = ids.contains(&item.id);
                    if ui.checkbox(&mut on, &item.name).changed() {
                        if on {
                            ids.push(item.id.clone());
                        } else {
                            ids.retain(|x| x != &item.id);
                        }
                    }
                }
                ui.add_space(space::S2);
                if st.tag_for != Some((d, i)) {
                    st.tag_text = tag.clone().unwrap_or_default();
                    st.tag_for = Some((d, i));
                }
                let field = ui.add(
                    egui::TextEdit::singleline(&mut st.tag_text)
                        .hint_text("tag, e.g. evening")
                        .desired_width(ui.available_width()),
                );
                if field.lost_focus() {
                    let typed = st.tag_text.trim();
                    *tag = (!typed.is_empty()).then(|| typed.to_string());
                }
                ui.add_space(space::S2);
                ui.horizontal(|ui| {
                    segmented(
                        ui,
                        theme,
                        order,
                        &[
                            (Order::Shuffle, "Shuffle", true),
                            (Order::Sequential, "In order", true),
                        ],
                    );
                });
            }
            Content::Random {
                archetypes,
                activities,
                weather,
            } => {
                group(ui, "Landscapes");
                filter_checks(ui, archetypes, &view::LANDSCAPES, &[]);
                group(ui, "Activities");
                filter_checks(ui, activities, &view::ACTIVITIES, &[]);
                group(ui, "Weather");
                filter_checks(ui, weather, &view::WEATHER, &view::CLEAR_WEATHER);
            }
        }

        group(ui, "Theme");
        ui.horizontal(|ui| {
            segmented(
                ui,
                theme,
                &mut s.theme,
                &[
                    (ThemeChoice::Follow, "Follow", true),
                    (ThemeChoice::Light, "Light", true),
                    (ThemeChoice::Dark, "Dark", true),
                ],
            )
        });
        group(ui, "Clawd");
        ui.horizontal(|ui| {
            segmented(
                ui,
                theme,
                &mut s.buddy,
                &[
                    (BuddyChoice::Follow, "Follow", true),
                    (BuddyChoice::On, "On", true),
                    (BuddyChoice::Off, "Off", true),
                ],
            )
        });
        group(ui, "Display");
        ui.horizontal(|ui| {
            segmented(
                ui,
                theme,
                &mut s.display,
                &[
                    (DisplayChoice::Follow, "Follow", true),
                    (DisplayChoice::Still, "Still", true),
                    (DisplayChoice::Live, "Live", false),
                ],
            )
        });
        if s != slot {
            commit(st, tl, actions, |tl| tl.slots_mut(d)[i] = s);
        }
        ui.add_space(space::S5);
        if ui.button("Delete").clicked() {
            commit(st, tl, actions, |tl| tl.remove(d, i));
            st.selected = None;
        }
    });
}

/// The editor driven by synthetic pointer and key input, headless: the same
/// code the window runs, checked through the timeline it leaves behind.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};
    use std::collections::BTreeMap;

    struct Harness {
        ctx: egui::Context,
        st: TimelineState,
        settings: Settings,
        status: view::Status,
        tl: Timeline,
        uses: BTreeMap<String, Vec<String>>,
        time: f64,
        changed: usize,
    }

    fn item(start: u16, id: &str) -> Slot {
        Slot {
            content: Content::Item { id: id.into() },
            ..Slot::random(start)
        }
    }

    impl Harness {
        fn new() -> Harness {
            let mut week: [Vec<Slot>; 7] = Default::default();
            week[0] = vec![item(420, "a"), item(1170, "b")];
            let ctx = egui::Context::default();
            style::apply(&ctx, Theme::Dark);
            let mut h = Harness {
                ctx,
                st: TimelineState::default(),
                settings: Settings::default(),
                status: view::Status::default(),
                tl: Timeline {
                    layout: Days::Weekly,
                    single: Vec::new(),
                    week,
                },
                uses: BTreeMap::new(),
                time: 0.0,
                changed: 0,
            };
            h.idle(3);
            h
        }

        fn frame(&mut self, events: Vec<Event>, _modifiers: Modifiers) {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 720.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            self.time += 1.0 / 60.0;
            let mut actions = Vec::new();
            let dir = Path::new("no-such-library");
            let mut model = super::super::Model {
                settings: &mut self.settings,
                status: &self.status,
                library: &[],
                library_dir: dir,
                uses: &self.uses,
                timeline: &mut self.tl,
                archive: &[],
                archive_dir: dir,
                current_entry: None,
            };
            let st = &mut self.st;
            // No painter here: release the frame's texture uploads unused.
            self.ctx
                .run_ui(input, |ui| {
                    show(ui, Theme::Dark, st, &mut model, &mut actions)
                })
                .drop_without_applying_deltas();
            self.changed += actions
                .iter()
                .filter(|a| **a == UiAction::TimelineChanged)
                .count();
        }

        fn idle(&mut self, frames: usize) {
            for _ in 0..frames {
                self.frame(Vec::new(), Modifiers::NONE);
            }
        }

        /// A point on a day's bar at a time of day.
        fn at(&self, day: usize, minute: u16) -> Pos2 {
            let bar = self.st.bars[day];
            pos2(
                bar.left() + bar.width() * f32::from(minute) / f32::from(DAY_MINUTES),
                bar.center().y,
            )
        }

        fn button(&mut self, pos: Pos2, pressed: bool) {
            self.frame(
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
                Modifiers::NONE,
            );
        }

        fn click(&mut self, pos: Pos2) {
            self.button(pos, true);
            self.button(pos, false);
        }

        /// Press, move in steps, release.
        fn drag(&mut self, from: Pos2, to: Pos2) {
            self.button(from, true);
            for k in 1..=10 {
                let p = from + (to - from) * (k as f32 / 10.0);
                self.frame(vec![Event::PointerMoved(p)], Modifiers::NONE);
            }
            self.button(to, false);
            self.idle(2);
        }

        fn key(&mut self, key: Key, modifiers: Modifiers) {
            let e = |pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            };
            self.frame(vec![e(true), e(false)], modifiers);
        }

        fn starts(&self, day: usize) -> Vec<u16> {
            self.tl.slots(day).iter().map(|s| s.start).collect()
        }
    }

    #[test]
    fn dragging_a_left_edge_moves_the_start_with_snapping() {
        let mut h = Harness::new();
        assert_eq!(h.st.bars.len(), 7);
        let from = h.at(0, 1170);
        let to = h.at(0, 1262) + vec2(0.0, 6.0);
        h.drag(from, to);
        assert_eq!(h.starts(0), [420, 1260]);
        assert_eq!(h.changed, 1);
        assert_eq!(h.st.selected, Some((0, 1)));
        // It can't pass the slot before it.
        let from = h.at(0, 1260);
        let to = h.at(0, 100);
        h.drag(from, to);
        assert_eq!(h.starts(0), [420, 421]);
    }

    #[test]
    fn delete_while_dragging_an_edge_neither_crashes_nor_moves_another_slot() {
        let mut h = Harness::new();
        // Select the last slot, start dragging its edge, press Delete.
        let from = h.at(0, 1170);
        h.click(from);
        h.button(from, true);
        for k in 1..=4 {
            h.frame(
                vec![Event::PointerMoved(from + vec2(k as f32 * 3.0, 0.0))],
                Modifiers::NONE,
            );
        }
        h.key(Key::Delete, Modifiers::NONE);
        h.frame(
            vec![Event::PointerMoved(from + vec2(20.0, 0.0))],
            Modifiers::NONE,
        );
        h.button(from + vec2(20.0, 0.0), false);
        h.idle(2);
        assert_eq!(h.starts(0).len(), 2, "Delete waits until the drag ends");
        // And the first slot's drag doesn't jump to the second one.
        let second = h.starts(0)[1];
        let a = h.at(0, 420);
        h.click(a);
        h.button(a, true);
        for k in 1..=4 {
            h.frame(
                vec![Event::PointerMoved(a + vec2(k as f32 * 3.0, 0.0))],
                Modifiers::NONE,
            );
        }
        h.key(Key::Delete, Modifiers::NONE);
        h.button(h.at(0, 600), false);
        h.idle(2);
        assert_eq!(h.starts(0)[1], second, "the other slot stays where it was");
    }

    #[test]
    fn double_clicking_splits_and_clicking_selects() {
        let mut h = Harness::new();
        // Monday 12:00 sits inside "a" (07:00-19:30).
        let p = h.at(0, 722);
        h.click(p);
        assert_eq!(h.st.selected, Some((0, 0)));
        h.click(p);
        assert_eq!(h.starts(0), [420, 720, 1170]);
        assert_eq!(h.tl.slots(0)[1].content, Content::Item { id: "a".into() });
        assert_eq!(h.st.selected, Some((0, 1)));
        // Tuesday is empty (the Generate schedule): a click adds a slot
        // there, as the ghost under the pointer shows.
        h.idle(30);
        let p = h.at(1, 302);
        h.click(p);
        assert_eq!(h.starts(1), [300]);
        assert_eq!(h.st.selected, Some((1, 0)));
    }

    #[test]
    fn dropping_a_random_scene_adds_a_slot_there() {
        let mut h = Harness::new();
        let from = h.st.random_chip.unwrap().center();
        let to = h.at(3, 1085);
        h.drag(from, to);
        assert_eq!(h.starts(3), [1080]);
        assert!(matches!(h.tl.slots(3)[0].content, Content::Random { .. }));
        assert_eq!(h.st.selected, Some((3, 0)));
    }

    #[test]
    fn keys_select_delete_and_undo() {
        let mut h = Harness::new();
        h.key(Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.st.selected, Some((0, 0)));
        h.key(Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.st.selected, Some((0, 1)));
        h.key(Key::Delete, Modifiers::NONE);
        assert_eq!(h.starts(0), [420]);
        assert_eq!(h.st.selected, None);
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.starts(0), [420, 1170]);
        assert_eq!(h.changed, 2);
    }
}
