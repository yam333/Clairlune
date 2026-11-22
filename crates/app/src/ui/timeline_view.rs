//! The timeline editor (PLAN.md section 9b, docs/design.md section 4): a
//! 24-hour bar for every day (one in the single layout, seven in the weekly
//! one), with slots as blocks showing their wallpaper. Saved wallpapers sit on
//! the left, ready to drag onto a day; the selected slot's choices are on the
//! right. Every drag has a keyboard and a button equivalent (WCAG 2.5.7):
//! typing a start or Earlier/Later for dragging an edge, Add for
//! double-clicking, and the slot's own picker for dropping a wallpaper.

use super::style::{self, c, colors};
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
const LABEL_W: f32 = 164.0;
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

/// Saved wallpapers, and a random scene, to drag onto a day.
fn sidebar(ui: &mut Ui, look: &Look, st: &mut TimelineState) {
    let t = colors(look.theme);
    ui.label(RichText::new("Add to a day").font(style::body_strong()));
    ui.label(RichText::new("Drag onto a day's bar.").color(c(t.text_secondary)));
    ui.add_space(space::S3);
    ui.dnd_drag_source(Id::unique("tl-drag-random"), Payload::Random, |ui| {
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), size::CONTROL + 8.0),
            Sense::hover(),
        );
        st.random_chip = Some(rect);
        ui.painter().rect(
            rect,
            radius::CONTROL,
            c(t.bg_raised),
            Stroke::new(size::HAIRLINE, c(t.border_control)),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.left_center() + vec2(space::S3, 0.0),
            Align2::LEFT_CENTER,
            "Random scene",
            style::body_medium(),
            c(t.text),
        );
    });
    ui.add_space(space::S4);
    ui.label(RichText::new("Saved").font(style::body_strong()));
    ui.add_space(space::S1);
    egui::ScrollArea::vertical().show(ui, |ui| {
        if look.items.is_empty() {
            ui.label(
                RichText::new(
                    "Wallpapers you save to the library show here. Random slots work without them.",
                )
                .color(c(t.text_secondary)),
            );
        }
        for item in look.items {
            ui.dnd_drag_source(
                Id::unique(("tl-drag", &item.id)),
                Payload::Item(item.id.clone()),
                |ui| {
                    ui.spacing_mut().item_spacing.y = space::S1;
                    let w = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
                    match st.thumbs.get(ui.ctx(), look.dir, &item.id) {
                        Some(tex) => egui::Image::new(tex)
                            .corner_radius(radius::CONTROL)
                            .paint_at(ui, rect),
                        None => {
                            ui.painter()
                                .rect_filled(rect, radius::CONTROL, c(t.bg_raised));
                        }
                    }
                    ui.label(RichText::new(&item.name).font(style::body_medium()));
                },
            );
            ui.add_space(space::S3);
        }
    });
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut layout = tl.layout;
            if segmented(
                ui,
                theme,
                &mut layout,
                &[
                    (Days::Single, "Every day", true),
                    (Days::Weekly, "Each weekday", true),
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
            ui.add_space(space::S4);
            if ui
                .add_enabled(!st.undo.is_empty(), egui::Button::new("Undo"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                undo(st, tl, actions);
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
                ui.label(
                    RichText::new("Which day should run every day?").font(style::body_strong()),
                );
                ui.label(
                    RichText::new("The other six days are dropped; Undo brings them back.")
                        .color(c(t.text_secondary)),
                );
                ui.add_space(space::S2);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("tl-keep")
                        .selected_text(day_name(keep))
                        .show_ui(ui, |ui| {
                            for d in 0..7 {
                                ui.selectable_value(&mut keep, d, day_name(d));
                            }
                        });
                    st.keep_day = Some(keep);
                    if primary_button(ui, theme, "Use it every day").clicked() {
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

    let running = settings.rotation.mode == RotationMode::Timeline;
    if !running {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Not running: Rotation is set to Generate.")
                    .color(c(t.text_secondary)),
            );
            if ui.button("Use the timeline").clicked() {
                settings.rotation.mode = RotationMode::Timeline;
                actions.push(UiAction::SettingsChanged);
            }
        });
    } else if tl.is_empty() {
        ui.label(
            RichText::new("No slots yet, so new scenes come on the Generate schedule.")
                .color(c(t.text_secondary)),
        );
    } else if let Some(a) =
        timeline::resolve(tl, &jiff::tz::TimeZone::system(), jiff::Timestamp::now())
    {
        let what = match a.pick {
            timeline::Pick::Slot { weekday, index } => tl
                .day(weekday)
                .get(index)
                .map(|s| describe(&s.content, look.items))
                .unwrap_or_default(),
            timeline::Pick::Generate => "new scenes on the Generate schedule".into(),
        };
        let local = timeline::local_hhmm;
        ui.label(
            RichText::new(format!(
                "Now: {what}, since {}. Next slot at {}.",
                local(a.since),
                local(a.next)
            ))
            .color(c(t.text_secondary)),
        );
    }
    for p in &status.timeline_problems {
        ui.label(RichText::new(p).color(c(t.danger)));
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
        day_actions(ui, look, st, tl, day, label, is_today, actions);
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

/// The day's name and its Add, Copy to... and Clear buttons.
#[allow(clippy::too_many_arguments)]
fn day_actions(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    day: usize,
    rect: Rect,
    is_today: bool,
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
            ui.label(
                RichText::new("today")
                    .font(style::mono())
                    .color(c(t.text_secondary)),
            );
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = 24.0;
        ui.spacing_mut().button_padding = vec2(space::S2, 2.0);
        let small = |s: &str| egui::Button::new(RichText::new(s).size(text::SMALL.size));
        if ui
            .add(small("Add"))
            .on_hover_text("Add a slot in the day's longest stretch")
            .clicked()
            && let Some(i) = commit(st, tl, actions, |tl| tl.add_slot(day))
        {
            st.selected = Some((day, i));
        }
        if tl.layout == Days::Weekly {
            let copy = ui.add(small("Copy to\u{2026}"));
            if copy.clicked() {
                st.copy_to = [false; 7];
            }
            egui::Popup::menu(&copy)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| copy_picker(ui, theme, st, tl, day, actions));
        }
        if ui
            .add_enabled(!tl.slots(day).is_empty(), small("Clear"))
            .clicked()
        {
            commit(st, tl, actions, |tl| tl.clear_day(day));
            if st.selected.is_some_and(|(d, _)| d == day) {
                st.selected = None;
            }
        }
    });
}

/// Copy a day over others, with quick picks; says which days it replaces.
fn copy_picker(
    ui: &mut Ui,
    theme: Theme,
    st: &mut TimelineState,
    tl: &mut Timeline,
    day: usize,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    ui.set_min_width(240.0);
    ui.label(RichText::new(format!("Copy {} to", day_name(day))).font(style::body_strong()));
    ui.horizontal(|ui| {
        if ui.button("All days").clicked() {
            st.copy_to = [true; 7];
        }
        if ui.button("Weekdays").clicked() {
            st.copy_to = [true, true, true, true, true, false, false];
        }
        if ui.button("Weekend").clicked() {
            st.copy_to = [false, false, false, false, false, true, true];
        }
    });
    for d in (0..7).filter(|d| *d != day) {
        ui.checkbox(&mut st.copy_to[d], day_name(d));
    }
    let targets: Vec<usize> = (0..7).filter(|d| *d != day && st.copy_to[*d]).collect();
    let replaced: Vec<&str> = targets
        .iter()
        .filter(|d| !tl.slots(**d).is_empty())
        .map(|d| day_name(*d))
        .collect();
    if !replaced.is_empty() {
        ui.label(
            RichText::new(format!("Replaces the slots on {}.", replaced.join(", ")))
                .color(c(t.danger)),
        );
    }
    let label = if replaced.is_empty() {
        "Copy"
    } else {
        "Copy and replace"
    };
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

    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let m = (minute_at(p.x).max(0.0) as u16).min(DAY_MINUTES - 1);
        st.selected = match tl.slot_at(day, m) {
            Some(i) => Some((day, i)),
            None if tl.slots(day).is_empty() => None,
            None => match tl.carried_into(day) {
                Carried::Slot(d, i) => Some((d, i)),
                Carried::Generate => None,
            },
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
    if slots.is_empty() {
        painter.rect(
            bar,
            radius::CONTROL,
            c(t.bg_sidebar),
            Stroke::new(size::HAIRLINE, c(t.hairline)),
            StrokeKind::Inside,
        );
        painter.text(
            bar.center(),
            Align2::CENTER_CENTER,
            "Empty: new scenes on the Generate schedule",
            style::body(),
            c(t.text_secondary),
        );
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
                Carried::Generate => {
                    painter.rect(
                        r,
                        radius::CONTROL,
                        c(t.bg_sidebar),
                        Stroke::new(size::HAIRLINE, c(t.hairline)),
                        StrokeKind::Inside,
                    );
                    if r.width() >= 80.0 {
                        painter.with_clip_rect(r).text(
                            r.left_center() + vec2(space::S2, 0.0),
                            Align2::LEFT_CENTER,
                            "Generate",
                            FontId::new(text::SMALL.size, FontFamily::Proportional),
                            c(t.text_secondary),
                        );
                    }
                }
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
        let missing = matches!(content, Content::Item { .. }) && tex.is_none();
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
        ui.label(RichText::new("Slot").font(style::title()));
        ui.add_space(space::S2);
        for line in [
            "Click a slot to change what it shows.",
            "Double-click a day to split it there, or use Add.",
            "Drag a slot's left edge to move its start; hold Shift for single minutes.",
            "Drag a saved wallpaper or a random scene onto a day to add a slot.",
            "Arrow keys pick slots, Delete removes one, Ctrl+Z undoes.",
        ] {
            ui.label(RichText::new(line).color(c(t.text_secondary)));
            ui.add_space(space::S1);
        }
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
            if ui.button("Earlier").clicked() {
                commit(st, tl, actions, |tl| {
                    tl.move_start(d, i, slot.start.saturating_sub(15))
                });
                moved = true;
            }
            if ui.button("Later").clicked() {
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
                ui.label(
                    RichText::new("Picks one of these each time the slot starts.")
                        .color(c(t.text_secondary)),
                );
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
                        .hint_text("And every item tagged, e.g. evening")
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
                if ids.is_empty() && tag.is_none() {
                    ui.label(
                        RichText::new("An empty pool shows a random scene.")
                            .color(c(t.text_secondary)),
                    );
                }
            }
            Content::Random {
                archetypes,
                activities,
                weather,
            } => {
                ui.label(
                    RichText::new("A new scene each time; all ticked means any.")
                        .color(c(t.text_secondary)),
                );
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
        ui.label(
            RichText::new("Live wallpapers come in a later version.").color(c(t.text_secondary)),
        );
        if s != slot {
            commit(st, tl, actions, |tl| tl.slots_mut(d)[i] = s);
        }
        ui.add_space(space::S5);
        if ui.button("Delete slot").clicked() {
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
        // Tuesday is empty (the Generate schedule): nothing to select.
        h.idle(30);
        let p = h.at(1, 300);
        h.click(p);
        assert_eq!(h.st.selected, None, "an empty day has nothing to select");
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
