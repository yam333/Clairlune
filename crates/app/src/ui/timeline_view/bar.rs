//! The days: the hour scale, each day's marks and its 24-hour bar, with
//! every pointer interaction.

use super::*;

/// The hour scale and one row per day.
pub(crate) fn days(
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
pub(crate) fn day_actions(
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
pub(crate) fn copy_picker(
    ui: &mut Ui,
    theme: Theme,
    st: &mut TimelineState,
    tl: &mut Timeline,
    day: usize,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        for (d, picked) in st.copy_to.iter_mut().enumerate() {
            let on = d == day || *picked;
            let replaced = d != day && *picked && !tl.slots(d).is_empty();
            if day_chip(ui, theme, short_day(d), on, replaced, d != day).clicked() {
                *picked = !*picked;
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
pub(crate) fn edge_at(starts: &[u16], bar: Rect, x: f32) -> Option<usize> {
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
pub(crate) fn day_bar(
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

/// One slot block: its wallpaper (or plain for random scenes), the start
/// and the name when they fit. `start` is `None` for the part carried over
/// from the day before, which is drawn faded.
pub(crate) fn block(
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
pub(crate) fn until(tl: &Timeline, day: usize, index: usize) -> String {
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
