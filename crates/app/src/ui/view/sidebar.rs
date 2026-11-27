//! The sidebar: the wordmark, the sections, and what's on screen now.

use super::*;

pub(crate) fn sidebar(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ViewState,
    settings: &Settings,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let g = &t.glass;
    // The wordmark: Clair, and lune in italic.
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        let (place, _) = ui.allocate_exact_size(vec2(20.0, 26.0), Sense::hover());
        mark(
            ui.painter(),
            Rect::from_center_size(place.center(), vec2(24.0, 24.0)),
            c(t.text),
            c(t.accent_deep),
        );
        ui.add_space(4.0);
        let mut job = egui::text::LayoutJob::default();
        for (text, italic) in [("Clair", false), ("lune", true)] {
            job.append(
                text,
                0.0,
                egui::TextFormat {
                    font_id: style::serif(crate::tokens::text::TITLE.size, italic),
                    color: c(t.text),
                    ..Default::default()
                },
            );
        }
        ui.label(job);
    });
    ui.add_space(space::S4 + 2.0);
    // Each section sits on a small pane of smoked glass, so its name reads
    // clearly over the flutes; the chosen one is lit.
    for s in Section::ALL {
        let selected = st.section == s;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, s.name()));
        let p = ui.painter();
        let (fill, edge) = if selected {
            (alpha(g.card, g.chosen), highlight(theme))
        } else if resp.hovered() {
            (
                alpha(g.pane, (g.nav + 0.15).min(1.0)),
                highlight(theme).gamma_multiply(0.6),
            )
        } else {
            (alpha(g.pane, g.nav), highlight(theme).gamma_multiply(0.35))
        };
        p.add(egui::Shape::convex_polygon(
            squircle(rect, 12.0),
            fill,
            Stroke::new(1.0, edge),
        ));
        if selected {
            p.circle_filled(
                pos2(rect.max.x - 14.0, rect.center().y),
                3.0,
                c(t.accent_deep),
            );
        }
        let ink = if selected {
            c(t.text)
        } else {
            c(t.text_secondary)
        };
        icon(
            p,
            Icon::Section(s),
            Rect::from_center_size(rect.left_center() + vec2(20.0, 0.0), vec2(16.0, 16.0)),
            ink,
        );
        let font = if selected {
            style::body_medium()
        } else {
            style::body()
        };
        p.text(
            rect.left_center() + vec2(38.0, 0.0),
            Align2::LEFT_CENTER,
            s.name(),
            font,
            ink,
        );
        if resp.has_focus() {
            focus_ring(ui, theme, rect, radius::CONTROL);
        }
        if resp.clicked() {
            st.section = s;
        }
        ui.add_space(3.0);
    }
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        now_showing(ui, theme, st, settings, status, actions);
    });
}

/// The wallpaper on screen at the sidebar's foot: a picture of it, what it
/// is, live or still, when the next comes; and the actions people reach for
/// most (in a bottom-up layout: the actions first).
pub(crate) fn now_showing(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ViewState,
    settings: &Settings,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let g = &t.glass;
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(w, size::BUTTON_PRIMARY),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_enabled_ui(!status.busy, |ui| {
                let icon_w = size::BUTTON_PRIMARY;
                if primary_button_wide(ui, theme, "New wallpaper", w - 2.0 * (icon_w + 6.0))
                    .clicked()
                {
                    actions.push(UiAction::NewWallpaper);
                }
                if icon_button(ui, theme, Icon::Previous, "Previous wallpaper").clicked() {
                    actions.push(UiAction::Previous);
                }
                if icon_button(ui, theme, Icon::Keep, "Save to library").clicked() {
                    actions.push(UiAction::SaveToLibrary);
                }
            });
        },
    );
    ui.add_space(space::S2);
    let pad = 10.0;
    // Below the picture: the title, a line of facts, the seed's box.
    let below = 8.0 + 20.0 + 17.0 + 6.0 + 22.0;
    let note_h = if status.note.is_some() { 48.0 } else { 0.0 };
    // A short window keeps the sections clear: the picture shrinks, or goes.
    let room = ui.available_height() - 2.0 * pad - below - note_h - space::S3;
    let pic_h = if room >= 56.0 { room.min(96.0) } else { 0.0 };
    let card_h = pad + pic_h + below + pad - if pic_h > 0.0 { 0.0 } else { 8.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(w, card_h), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, radius::PANEL, alpha(g.card, g.card_alpha * 1.3));
    p.hline(
        rect.x_range().shrink(radius::PANEL),
        rect.min.y + 0.5,
        Stroke::new(1.0, highlight(theme)),
    );
    let pic = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(w - 2.0 * pad, pic_h));
    if pic_h > 0.0 {
        match &st.thumb {
            Some(tex) => {
                let tint = if status.busy {
                    Color32::from_white_alpha(150)
                } else {
                    Color32::WHITE
                };
                egui::Image::new(tex)
                    .uv(cover_uv(tex.size_vec2(), pic.size()))
                    .corner_radius(8)
                    .tint(tint)
                    .paint_at(ui, pic);
            }
            None => {
                p.rect_filled(pic, 8.0, alpha(g.card, g.card_alpha * 2.0));
                mark(
                    &p,
                    Rect::from_center_size(pic.center(), vec2(36.0, 36.0)),
                    c(t.text_secondary),
                    c(t.accent_deep),
                );
            }
        }
    }
    let what = if status.busy {
        "Making a new wallpaper".to_string()
    } else if status.what.is_empty() {
        "No wallpaper yet".to_string()
    } else {
        capitalise(&status.what)
    };
    let _ = settings;
    let live = status.live_active;
    let (tag, tag_ink) = if live {
        ("LIVE", c(t.accent_text))
    } else {
        ("STILL", c(t.text_secondary))
    };
    let tag_g = p.layout_no_wrap(tag.into(), style::label(), tag_ink);
    let y = if pic_h > 0.0 {
        pic.max.y + 8.0
    } else {
        pic.min.y
    };
    // Room for the tag and, when live, its dot.
    let title_w = pic.width() - tag_g.size().x - if live { 24.0 } else { 12.0 };
    marquee(
        ui,
        &p,
        Rect::from_min_size(pos2(pic.min.x + 2.0, y), vec2(title_w, 20.0)),
        &what,
        c(t.text),
    );
    let tag_x = pic.max.x - tag_g.size().x - 2.0;
    if live {
        p.circle_filled(pos2(tag_x - 7.0, y + 9.0), 3.0, c(t.accent_text));
    }
    p.galley(pos2(tag_x, y + 3.0), tag_g, tag_ink);
    let next = if status.paused {
        "next: paused".to_string()
    } else {
        match status.next_in_minutes {
            None if status.timeline_next.is_some() => {
                let at = status.timeline_next.clone().unwrap_or_default();
                format!("next: {}", at.strip_prefix("Next slot at ").unwrap_or(&at))
            }
            Some(0) => "next: under a minute".to_string(),
            Some(m) if m < 120 => format!("next: {m} min"),
            Some(m) => format!("next: {} h", m / 60),
            None => "next: when you ask".to_string(),
        }
    };
    let facts = match status.theme {
        Some(th) => format!("{} \u{b7} {next}", th.as_str()),
        None => next,
    };
    let gal = p.layout_job(one_line(
        &facts,
        style::mono(),
        c(t.text_secondary),
        pic.width(),
    ));
    p.galley(pos2(pic.min.x + 2.0, y + 22.0), gal, c(t.text_secondary));
    if status.is_photo {
        let at = Rect::from_min_size(pos2(pic.min.x, y + 45.0), vec2(pic.width(), 22.0));
        photo_badge(theme, &p, at);
    } else if let Some(seed) = status.seed {
        let at = Rect::from_min_size(pos2(pic.min.x, y + 45.0), vec2(pic.width(), 22.0));
        seed_box(ui, theme, at, seed, &mut st.page);
    }
    if let Some(note) = &status.note {
        ui.add_space(space::S2);
        ui.label(RichText::new(note).small().color(c(t.text_secondary)));
    }
}

/// Quiet non-interactive badge shown instead of a seed box when a custom ASCII
/// image is on screen.
fn photo_badge(theme: Theme, p: &Painter, rect: Rect) {
    let t = colors(theme);
    let g = &t.glass;
    p.add(egui::Shape::convex_polygon(
        squircle(rect, 8.0),
        alpha(g.card, g.card_alpha * 1.6),
        Stroke::new(1.0, alpha(t.border_control, 0.3)),
    ));
    let inner = rect.shrink2(vec2(8.0, 0.0));
    let gal = p.layout_job(one_line(
        "custom image",
        style::label(),
        c(t.text_secondary),
        inner.width(),
    ));
    let text_y = rect.center().y - gal.size().y / 2.0;
    p.galley(pos2(inner.min.x, text_y), gal, c(t.text_secondary));
}

/// The seed in a small box: hovered it offers to copy, clicked it copies.
pub(crate) fn seed_box(ui: &mut Ui, theme: Theme, rect: Rect, seed: u64, ps: &mut PageState) {
    let t = colors(theme);
    let g = &t.glass;
    let id = ui.scope_id().with("seed-box");
    let resp = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Copy the seed"));
    let now = ui.input(|i| i.time);
    if resp.clicked() {
        ui.ctx().copy_text(seed.to_string());
        ui.ctx().data_mut(|d| d.insert_temp(id, now));
        ps.toast = Some("Copied the seed".into());
    }
    let copied = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(id))
        .is_some_and(|at| now - at < 1.5);
    if copied {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
    let shown = resp.hovered() || resp.has_focus() || copied;
    let p = ui.painter();
    let fill = if shown {
        alpha(g.card, g.chosen)
    } else {
        alpha(g.card, g.card_alpha * 1.6)
    };
    let edge = alpha(t.border_control, if shown { 0.6 } else { 0.3 });
    p.add(egui::Shape::convex_polygon(
        squircle(rect, 8.0),
        fill,
        Stroke::new(1.0, edge),
    ));
    let inner = rect.shrink2(vec2(8.0, 0.0));
    let action = if copied { "copied" } else { "copy" };
    let action_g = p.layout_no_wrap(action.into(), style::label(), c(t.accent_text));
    let room = if shown {
        inner.width() - action_g.size().x - 8.0
    } else {
        inner.width()
    };
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = room;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    for (text, ink) in [("seed ", t.text_secondary), (&*seed.to_string(), t.text)] {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: style::label(),
                color: c(ink),
                ..Default::default()
            },
        );
    }
    let text = p.layout_job(job);
    let text_y = rect.center().y - text.size().y / 2.0;
    p.galley(pos2(inner.min.x, text_y), text, c(t.text));
    if shown {
        let ax = inner.max.x - action_g.size().x;
        let ay = rect.center().y - action_g.size().y / 2.0;
        p.galley(pos2(ax, ay), action_g, c(t.accent_text));
    }
    if resp.has_focus() {
        focus_ring(ui, theme, rect, 8.0);
    }
}

/// A box whose corners bend like a superellipse (|x|^4 + |y|^4 = 1) rather
/// than a quarter circle: the curve leaves the straight sides gently, the
/// way a squircle does. `r` is how far each corner reaches; the points run
/// clockwise.
pub(crate) fn squircle(rect: Rect, r: f32) -> Vec<egui::Pos2> {
    const STEPS: usize = 10;
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let centres = [
        pos2(rect.max.x - r, rect.min.y + r),
        pos2(rect.max.x - r, rect.max.y - r),
        pos2(rect.min.x + r, rect.max.y - r),
        pos2(rect.min.x + r, rect.min.y + r),
    ];
    let mut out: Vec<egui::Pos2> = Vec::with_capacity(4 * (STEPS + 1));
    for (k, centre) in centres.into_iter().enumerate() {
        for i in 0..=STEPS {
            let a = (k as f32 - 1.0 + i as f32 / STEPS as f32) * std::f32::consts::FRAC_PI_2;
            let (sin, cos) = a.sin_cos();
            let at = centre
                + r * vec2(
                    cos.signum() * cos.abs().sqrt(),
                    sin.signum() * sin.abs().sqrt(),
                );
            if out.last().is_none_or(|l| l.distance(at) > 0.01) {
                out.push(at);
            }
        }
    }
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) <= 0.01 {
        out.pop();
    }
    out
}

/// A line of text in `rect`: as it is when it fits, otherwise drifting
/// slowly left while the window is focused, resting a moment at the start of
/// each pass.
pub(crate) fn marquee(ui: &Ui, p: &Painter, rect: Rect, text: &str, ink: Color32) {
    const SPEED: f32 = 18.0; // points a second
    const GAP: f32 = 40.0;
    const REST: f32 = 2.5; // seconds
    let galley = p.layout_no_wrap(text.to_string(), style::body_medium(), ink);
    let w = galley.size().x;
    if w <= rect.width() {
        p.galley(rect.min, galley, ink);
        return;
    }
    let clip = p.with_clip_rect(rect.expand2(vec2(0.0, 3.0)).intersect(p.clip_rect()));
    if !ui.input(|i| i.focused) {
        clip.galley(rect.min, galley, ink);
        return;
    }
    let span = w + GAP;
    let cycle = REST + span / SPEED;
    let phase = (ui.input(|i| i.time) as f32) % cycle;
    let shift = ((phase - REST).max(0.0) * SPEED).round();
    clip.galley(rect.min - vec2(shift, 0.0), galley.clone(), ink);
    clip.galley(rect.min + vec2(span - shift, 0.0), galley, ink);
    let next = if phase < REST {
        std::time::Duration::from_secs_f32((REST - phase).max(0.05))
    } else {
        std::time::Duration::from_millis(55)
    };
    ui.ctx().request_repaint_after(next);
}

/// One line of text, cut short with an ellipsis where it doesn't fit.
pub(crate) fn one_line(
    text: &str,
    font: egui::FontId,
    color: Color32,
    width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, width);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job
}

/// The part of an image of `tex` size that covers a box of `target` size,
/// centred.
pub(crate) fn cover_uv(tex: egui::Vec2, target: egui::Vec2) -> Rect {
    let (ta, ba) = (tex.x / tex.y.max(1.0), target.x / target.y.max(1.0));
    if ta > ba {
        let w = ba / ta;
        Rect::from_min_max(pos2(0.5 - w / 2.0, 0.0), pos2(0.5 + w / 2.0, 1.0))
    } else {
        let h = ta / ba;
        Rect::from_min_max(pos2(0.0, 0.5 - h / 2.0), pos2(1.0, 0.5 + h / 2.0))
    }
}

/// The light along a glass card's top edge.
pub(crate) fn highlight(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_white_alpha(38),
        Theme::Light => Color32::from_white_alpha(220),
    }
}

pub(crate) fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
