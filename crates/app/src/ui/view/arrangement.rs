//! The monitors as a spread scene sees them (Display, Spread across): each
//! drawn where it sits, showing its part of the wallpaper on screen. Drag a
//! monitor to move it (it snaps to the others' edges; a gap stays in the
//! scene, as the frames between screens do). The bar on the main monitor,
//! where the scene centres, drags to another one, as the menu bar does in
//! the system's display settings; a double-click does the same.

use super::*;
use crate::span::{self, Placed};

/// A drag in progress on the arrangement.
#[derive(Clone, Debug)]
pub(crate) struct ArrangeDrag {
    /// The monitor being moved, or whose bar is being moved.
    id: String,
    /// The bar (the main monitor), not the monitor itself.
    bar: bool,
    /// Where on the monitor it was picked up, in points from its corner.
    grab: egui::Vec2,
}

/// How far from an edge (in points on screen) a dragged monitor snaps.
const SNAP: f32 = 10.0;
/// The panel's height.
const PANEL_H: f32 = 260.0;

/// The arrangement scaled into the panel.
pub(crate) struct Geometry {
    k: f32,
    x0: f32,
    y0: f32,
    origin: Pos2,
}

impl Geometry {
    pub(crate) fn new(panel: Rect, placed: &[Placed]) -> Geometry {
        let x0 = placed.iter().map(|m| m.x).min().unwrap_or(0) as f32;
        let y0 = placed.iter().map(|m| m.y).min().unwrap_or(0) as f32;
        let x1 = placed.iter().map(|m| m.x + m.w as i32).max().unwrap_or(1) as f32;
        let y1 = placed.iter().map(|m| m.y + m.h as i32).max().unwrap_or(1) as f32;
        let inner = panel.shrink(28.0);
        let k = (inner.width() / (x1 - x0).max(1.0)).min(inner.height() / (y1 - y0).max(1.0));
        let origin = inner.center() - vec2(x1 - x0, y1 - y0) * k / 2.0;
        Geometry { k, x0, y0, origin }
    }

    /// A monitor at (`x`, `y`) in arrangement units, on screen.
    pub(crate) fn rect(&self, x: i32, y: i32, m: &Placed) -> Rect {
        Rect::from_min_size(
            self.origin + vec2((x as f32 - self.x0) * self.k, (y as f32 - self.y0) * self.k),
            vec2(m.w as f32 * self.k, m.h as f32 * self.k),
        )
    }

    /// A point on screen in arrangement units.
    fn units(&self, at: Pos2) -> (f32, f32) {
        let v = at - self.origin;
        (v.x / self.k + self.x0, v.y / self.k + self.y0)
    }
}

pub(crate) fn arrangement(
    ui: &mut Ui,
    theme: Theme,
    s: &mut Settings,
    status: &Status,
    ps: &mut PageState,
    wallpaper: Option<&egui::TextureHandle>,
) {
    let t = colors(theme);
    let g = &t.glass;
    let placed = span::arrange(&status.screens, &s.display);
    if placed.len() < 2 {
        return;
    }
    let label = |id: &str| {
        status
            .monitors
            .iter()
            .find(|(m, _)| m == id)
            .map(|(_, l)| l.clone())
            .unwrap_or_default()
    };
    // The panel, and the arrangement scaled into it (from what's set, so the
    // scale holds still during a drag).
    let w = ui.available_width();
    let (panel, _) = ui.allocate_exact_size(vec2(w, PANEL_H), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(panel, radius::PANEL, alpha(g.card, g.card_alpha));
    p.rect_stroke(
        panel,
        radius::PANEL,
        Stroke::new(1.0, alpha(t.border_control, 0.3)),
        egui::StrokeKind::Inside,
    );
    let geo = Geometry::new(panel, &placed);
    let on_screen = |x: i32, y: i32, m: &Placed| geo.rect(x, y, m);
    // Each monitor's part of the scene on screen, when that's a spread.
    let spread = span::spread(&placed);
    let uv_of = |i: usize| {
        let v = spread.views[i];
        Rect::from_min_max(
            pos2(
                v.x as f32 / spread.width as f32,
                v.y as f32 / spread.height as f32,
            ),
            pos2(
                (v.x + v.w) as f32 / spread.width as f32,
                (v.y + v.h) as f32 / spread.height as f32,
            ),
        )
    };
    let pointer = ui.ctx().pointer_latest_pos();
    // Where the monitor being dragged would land.
    let moving = ps.arrange_drag.as_ref().filter(|d| !d.bar).and_then(|d| {
        let i = placed.iter().position(|m| m.id == d.id)?;
        let (x, y) = geo.units(pointer? - d.grab);
        let reach = (SNAP / geo.k) as i32;
        Some((
            i,
            span::snap(&placed, i, x.round() as i32, y.round() as i32, reach),
        ))
    });
    let mut rects = Vec::new();
    for (i, m) in placed.iter().enumerate() {
        let (x, y) = match moving {
            Some((j, at)) if j == i => at,
            _ => (m.x, m.y),
        };
        rects.push(on_screen(x, y, m));
    }
    // Monitors first, the dragged one last (on top); their bars after.
    let mut order: Vec<usize> = (0..placed.len()).collect();
    if let Some((j, _)) = moving {
        order.retain(|i| *i != j);
        order.push(j);
    }
    let mut responses = vec![None; placed.len()];
    for &i in &order {
        let m = &placed[i];
        let r = rects[i];
        let id = ui.make_persistent_id(("arrange", &m.id));
        let resp = ui
            .interact(r, id, Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(
                "Drag to line it up with the others. Double-click: the scene centres here.",
            );
        let lifted = moving.is_some_and(|(j, _)| j == i);
        if lifted {
            p.rect_filled(
                r.translate(vec2(0.0, 4.0)),
                6.0,
                Color32::from_black_alpha(70),
            );
        }
        match (wallpaper, status.span_shown) {
            (Some(tex), true) => {
                egui::Image::new(tex)
                    .uv(uv_of(i))
                    .corner_radius(5)
                    .paint_at(ui, r);
            }
            _ => {
                p.rect_filled(r, 5.0, c(t.bg_raised));
            }
        }
        let edge = if lifted || resp.hovered() {
            Stroke::new(2.0, c(t.accent_text))
        } else {
            Stroke::new(1.0, alpha(t.text, 0.45))
        };
        p.rect_stroke(r, 5.0, edge, egui::StrokeKind::Inside);
        // Its name, small, on a scrim.
        let name = label(&m.id);
        let name = name.split(" (").next().unwrap_or("").to_string();
        if !name.is_empty() && r.width() > 44.0 {
            let gal = p.layout_no_wrap(name, egui::FontId::proportional(11.0), c(t.text));
            let at = pos2(r.left() + 6.0, r.bottom() - gal.size().y - 6.0);
            p.rect_filled(
                Rect::from_min_size(at - vec2(4.0, 1.0), gal.size() + vec2(8.0, 2.0)),
                4.0,
                c(t.bg).gamma_multiply(0.8),
            );
            p.galley(at, gal, c(t.text));
        }
        responses[i] = Some(resp);
    }
    // The main monitor's bar, unless it's being carried elsewhere.
    let main = placed.iter().position(|m| m.main).unwrap_or(0);
    let bar_of = |r: Rect| Rect::from_min_size(r.min + vec2(4.0, 4.0), vec2(r.width() - 8.0, 6.0));
    let carrying = ps.arrange_drag.as_ref().is_some_and(|d| d.bar);
    let bar = bar_of(rects[main]);
    let bar_resp = ui
        .interact(
            bar.expand(4.0),
            ui.make_persistent_id("arrange-bar"),
            Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::Grab)
        .on_hover_text("The scene centres on this monitor: drag the bar to another.");
    let bar_ink = |strong: bool| alpha(t.text, if strong { 0.95 } else { 0.8 });
    if !carrying {
        p.rect_filled(bar, 3.0, bar_ink(bar_resp.hovered()));
    }
    if bar_resp.drag_started() {
        ps.arrange_drag = Some(ArrangeDrag {
            id: placed[main].id.clone(),
            bar: true,
            grab: vec2(0.0, 0.0),
        });
    }
    if carrying && let Some(at) = pointer {
        // Over a monitor: the bar shows where it would go.
        let (at_bar, ink) = match rects.iter().position(|r| r.contains(at)) {
            Some(i) => (bar_of(rects[i]), c(t.accent_text)),
            None => (
                Rect::from_center_size(at, vec2(bar.width().min(80.0), 6.0)),
                bar_ink(true),
            ),
        };
        p.rect_filled(at_bar, 3.0, ink);
    }
    if bar_resp.drag_stopped() {
        if let Some(i) = pointer.and_then(|at| rects.iter().position(|r| r.contains(at))) {
            s.display.span_main = Some(placed[i].id.clone());
        }
        ps.arrange_drag = None;
    }
    // Monitors: double-click to centre the scene there; drag to move.
    for (i, resp) in responses.into_iter().enumerate() {
        let Some(resp) = resp else { continue };
        if resp.double_clicked() {
            s.display.span_main = Some(placed[i].id.clone());
        }
        // Held where it was pressed, not where the drag was noticed (a few
        // points later).
        if resp.drag_started()
            && !carrying
            && let Some(at) = ui.input(|inp| inp.pointer.press_origin())
        {
            ps.arrange_drag = Some(ArrangeDrag {
                id: placed[i].id.clone(),
                bar: false,
                grab: at - rects[i].min,
            });
        }
        if resp.drag_stopped()
            && let Some((j, (x, y))) = moving
            && j == i
        {
            // The whole arrangement is kept, so the others stay put if the
            // system's arrangement changes later.
            for (k2, m) in placed.iter().enumerate() {
                let at = if k2 == j { [x, y] } else { [m.x, m.y] };
                let name = label(&m.id);
                s.edit_monitor(&m.id, &name, |ms| ms.span_at = Some(at));
            }
            ps.arrange_drag = None;
        }
    }
    // Back to the system's arrangement.
    if span::customised(&s.display) {
        let reset = Rect::from_min_size(
            pos2(panel.right() - 34.0, panel.top() + 6.0),
            vec2(28.0, 28.0),
        );
        let resp = ui
            .interact(
                reset,
                ui.make_persistent_id("arrange-reset"),
                Sense::click(),
            )
            .on_hover_text("Back to the system's arrangement");
        if resp.hovered() {
            p.rect_filled(reset, 7.0, alpha(t.text, 0.08));
        }
        icon(
            &p,
            Icon::Previous,
            reset.shrink(6.0),
            if resp.hovered() {
                c(t.text)
            } else {
                c(t.text_secondary)
            },
        );
        if resp.clicked() {
            s.display.span_main = None;
            for m in &placed {
                let name = label(&m.id);
                s.edit_monitor(&m.id, &name, |ms| ms.span_at = None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};

    struct Harness {
        ctx: egui::Context,
        settings: Settings,
        status: Status,
        ps: PageState,
        panel: Rect,
        time: f64,
    }

    impl Harness {
        fn new() -> Harness {
            let mon = |id: &str, x: i32, y: i32, w: u32, h: u32, primary: bool, scale: u32| {
                clairlune_platform::Monitor {
                    id: id.into(),
                    x,
                    y,
                    width: w,
                    height: h,
                    primary,
                    scale,
                }
            };
            let screens = vec![
                mon("main", 0, 0, 2560, 1440, true, 1),
                mon("right", 2560, 0, 1920, 1080, false, 1),
                mon("laptop", 524, 1440, 3024, 1964, false, 2),
            ];
            let mut settings = Settings::default();
            settings.display.span = true;
            let ctx = egui::Context::default();
            style::apply(&ctx, Theme::Dark);
            let mut h = Harness {
                ctx,
                settings,
                status: Status {
                    monitors: screens
                        .iter()
                        .map(|m| (m.id.clone(), m.id.clone()))
                        .collect(),
                    screens,
                    ..Status::default()
                },
                ps: PageState::default(),
                panel: Rect::NOTHING,
                time: 0.0,
            };
            h.frame(Vec::new());
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(700.0, 400.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            self.time += 1.0 / 60.0;
            let (settings, status, ps) = (&mut self.settings, &self.status, &mut self.ps);
            let mut panel = Rect::NOTHING;
            self.ctx
                .run_ui(input, |ui| {
                    let at = ui.available_rect_before_wrap();
                    panel = Rect::from_min_size(at.min, vec2(at.width(), PANEL_H));
                    arrangement(ui, Theme::Dark, settings, status, ps, None);
                })
                .drop_without_applying_deltas();
            self.panel = panel;
        }

        /// Where a monitor is on screen now.
        fn rect(&self, id: &str) -> Rect {
            let placed = span::arrange(&self.status.screens, &self.settings.display);
            let geo = Geometry::new(self.panel, &placed);
            let m = placed.iter().find(|m| m.id == id).unwrap();
            geo.rect(m.x, m.y, m)
        }

        fn button(&mut self, at: Pos2, pressed: bool) {
            self.frame(vec![
                Event::PointerMoved(at),
                Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }

        fn drag(&mut self, from: Pos2, to: Pos2) {
            self.button(from, true);
            for k in 1..=8 {
                self.frame(vec![Event::PointerMoved(
                    from + (to - from) * (k as f32 / 8.0),
                )]);
            }
            self.button(to, false);
            self.frame(Vec::new());
        }

        fn at(&self, id: &str) -> Option<[i32; 2]> {
            self.settings.monitor(id).and_then(|m| m.span_at)
        }
    }

    #[test]
    fn dragging_a_monitor_moves_it_and_keeps_the_arrangement() {
        let mut h = Harness::new();
        let r = h.rect("right");
        // Down and away from the others: it moves there (no snap in reach),
        // and every monitor's place is kept.
        h.drag(r.center(), r.center() + vec2(40.0, 70.0));
        let [x, y] = h.at("right").expect("moved");
        assert!(x > 2560 && y > 0, "({x}, {y})");
        assert_eq!(h.at("main"), Some([0, 0]));
        assert_eq!(h.at("laptop"), Some([524, 1440]));
        // Dropped just off the main monitor's edge: it snaps back to touch it.
        let r = h.rect("right");
        let main = h.rect("main");
        let target = pos2(
            main.right() + r.width() / 2.0 + 4.0,
            main.top() + r.height() / 2.0 + 3.0,
        );
        h.drag(r.center(), target);
        assert_eq!(h.at("right"), Some([2560, 0]));
    }

    #[test]
    fn the_main_monitor_moves_by_its_bar_or_a_double_click() {
        let mut h = Harness::new();
        // The bar sits at the top of the main monitor: carry it to the laptop.
        let main = h.rect("main");
        let bar = pos2(main.center().x, main.top() + 7.0);
        h.drag(bar, h.rect("laptop").center());
        assert_eq!(h.settings.display.span_main.as_deref(), Some("laptop"));
        // A double-click on another monitor makes that one main.
        let c = h.rect("right").center();
        h.button(c, true);
        h.button(c, false);
        h.button(c, true);
        h.button(c, false);
        h.frame(Vec::new());
        assert_eq!(h.settings.display.span_main.as_deref(), Some("right"));
        // Back to the system's arrangement.
        let reset = pos2(h.panel.right() - 20.0, h.panel.top() + 20.0);
        h.button(reset, true);
        h.button(reset, false);
        h.frame(Vec::new());
        assert_eq!(h.settings.display.span_main, None);
        assert!(!span::customised(&h.settings.display));
    }
}
