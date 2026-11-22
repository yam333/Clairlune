//! The timeline editor (docs/dev/design.md section 4): a
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

mod bar;
mod demo;
mod inspector;
mod marks;
pub(crate) use bar::*;
pub(crate) use demo::*;
pub(crate) use inspector::*;
pub(crate) use marks::*;

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
pub(crate) struct Look<'a> {
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
