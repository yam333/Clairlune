//! With nothing selected: how the editor works, shown as a looping demo
//! and the keys drawn as keys.

use super::*;

/// What the editor does, shown rather than told (the way macOS's trackpad
/// settings show each gesture): a pointer drops a wallpaper on a day, moves
/// a slot's start, then opens a slot, over and over; and the keys, as keys.
pub(crate) fn demo(ui: &mut Ui, look: &Look, st: &mut TimelineState) {
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
pub(crate) fn keycap(ui: &mut Ui, theme: Theme, label: Option<&str>, job: &str) {
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
pub(crate) fn pointer(p: &egui::Painter, at: egui::Pos2, pressed: bool) {
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
pub(crate) fn resize_pointer(p: &egui::Painter, at: egui::Pos2) {
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
