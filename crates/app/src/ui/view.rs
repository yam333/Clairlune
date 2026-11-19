//! The Settings window's content: a sidebar of sections (PLAN.md section 8),
//! the current wallpaper as a status strip above every page, and rows of
//! name, one-line description and control (docs/design.md sections 3-4).

use super::style::{self, c, colors};
use crate::settings::{Settings, ThemeMode};
use crate::tokens::{radius, size, space};
use clairlune_scene::Theme;
use egui::{Align, Align2, Layout, Rect, RichText, Sense, Stroke, Ui, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Schedule,
    Theme,
    Display,
    Live,
    Buddy,
    Content,
    Archive,
    Export,
    General,
}

impl Section {
    pub const ALL: [Section; 9] = [
        Section::Schedule,
        Section::Theme,
        Section::Display,
        Section::Live,
        Section::Buddy,
        Section::Content,
        Section::Archive,
        Section::Export,
        Section::General,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::Schedule => "Schedule",
            Section::Theme => "Theme",
            Section::Display => "Display",
            Section::Live => "Live",
            Section::Buddy => "Buddy",
            Section::Content => "Content",
            Section::Archive => "Archive",
            Section::Export => "Export",
            Section::General => "General",
        }
    }
}

/// What the app shows about the wallpaper on screen.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// e.g. "stroll in cave".
    pub what: String,
    pub seed: Option<u64>,
    pub theme: Option<Theme>,
    /// Minutes until the next scheduled change.
    pub next_in_minutes: Option<u64>,
    pub busy: bool,
}

/// Requests from the window to the app.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    SettingsChanged,
    NewWallpaper,
    Previous,
    OpenArchive,
}

pub struct ViewState {
    pub section: Section,
    pub thumb: Option<egui::TextureHandle>,
}

const INTERVALS: [(u32, &str); 8] = [
    (15, "15 minutes"),
    (30, "30 minutes"),
    (60, "1 hour"),
    (180, "3 hours"),
    (360, "6 hours"),
    (720, "12 hours"),
    (1440, "1 day"),
    (0, "Only on demand"),
];

pub fn show(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ViewState,
    settings: &mut Settings,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    egui::Panel::left("sections")
        .exact_size(size::SIDEBAR)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_sidebar))
                .inner_margin(egui::Margin::symmetric(space::S3 as i8, space::S4 as i8)),
        )
        .show(ui, |ui| sidebar(ui, theme, st));
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(c(t.bg))
                .inner_margin(egui::Margin::symmetric(space::S6 as i8, space::S5 as i8)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(size::PAGE_MAX);
                status_strip(ui, theme, st, status, actions);
                ui.add_space(space::S6);
                page(ui, theme, st.section, settings, actions);
            });
        });
}

fn sidebar(ui: &mut Ui, theme: Theme, st: &mut ViewState) {
    let t = colors(theme);
    ui.add_space(space::S2);
    for s in Section::ALL {
        let selected = st.section == s;
        let (rect, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), size::CONTROL + 4.0),
            Sense::click(),
        );
        if resp.hovered() && !selected {
            ui.painter()
                .rect_filled(rect, radius::CONTROL, c(t.bg).gamma_multiply(0.6));
        }
        if selected {
            ui.painter().rect_filled(rect, radius::CONTROL, c(t.bg));
            let bar =
                Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(2.0, rect.height() - 12.0));
            ui.painter().rect_filled(bar, 1.0, c(t.accent));
        }
        let (font, color) = if selected {
            (style::body_medium(), c(t.text))
        } else {
            (style::body(), c(t.text_secondary))
        };
        ui.painter().text(
            rect.left_center() + vec2(space::S3, 0.0),
            Align2::LEFT_CENTER,
            s.name(),
            font,
            color,
        );
        if resp.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(1.0),
                radius::CONTROL,
                Stroke::new(size::FOCUS_RING, c(t.accent)),
                egui::StrokeKind::Outside,
            );
        }
        if resp.clicked() {
            st.section = s;
        }
    }
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.label(
            RichText::new(format!("Clairlune {}", env!("CARGO_PKG_VERSION")))
                .font(style::mono())
                .color(c(t.text_secondary)),
        );
    });
}

/// The wallpaper on screen, its seed and theme, the next change, and the two
/// actions people reach for most.
fn status_strip(
    ui: &mut Ui,
    theme: Theme,
    st: &ViewState,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    ui.horizontal_top(|ui| {
        let w = 256.0;
        let h = w * 9.0 / 16.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        match &st.thumb {
            Some(tex) => {
                let tint = if status.busy {
                    egui::Color32::from_white_alpha(150)
                } else {
                    egui::Color32::WHITE
                };
                egui::Image::new(tex)
                    .corner_radius(radius::PANEL)
                    .tint(tint)
                    .paint_at(ui, rect);
            }
            None => {
                ui.painter()
                    .rect_filled(rect, radius::PANEL, c(t.bg_raised));
            }
        }
        ui.painter().rect_stroke(
            rect,
            radius::PANEL,
            Stroke::new(size::HAIRLINE, c(t.hairline)),
            egui::StrokeKind::Inside,
        );
        ui.add_space(space::S5);
        ui.vertical(|ui| {
            let what = if status.busy {
                "Making a new wallpaper".to_string()
            } else if status.what.is_empty() {
                "No wallpaper yet".to_string()
            } else {
                capitalise(&status.what)
            };
            ui.label(RichText::new(what).font(style::body_strong()));
            let mut facts = Vec::new();
            if let Some(s) = status.seed {
                facts.push(format!("seed {s}"));
            }
            if let Some(th) = status.theme {
                facts.push(th.as_str().to_string());
            }
            if !facts.is_empty() {
                ui.label(
                    RichText::new(facts.join("  "))
                        .font(style::mono())
                        .color(c(t.text_secondary)),
                );
            }
            let next = match status.next_in_minutes {
                Some(0) => "Next change in under a minute".to_string(),
                Some(1) => "Next change in 1 minute".to_string(),
                Some(m) if m < 120 => format!("Next change in {m} minutes"),
                Some(m) => format!("Next change in {} hours", m / 60),
                None => "Changes only on demand".to_string(),
            };
            ui.label(RichText::new(next).color(c(t.text_secondary)));
            ui.add_space(space::S3);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!status.busy, |ui| {
                    if primary_button(ui, theme, "New wallpaper").clicked() {
                        actions.push(UiAction::NewWallpaper);
                    }
                    if ui.button("Previous").clicked() {
                        actions.push(UiAction::Previous);
                    }
                });
            });
        });
    });
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn primary_button(ui: &mut Ui, theme: Theme, label: &str) -> egui::Response {
    let t = colors(theme);
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .color(c(t.on_accent))
                .font(style::body_medium()),
        )
        .fill(c(t.accent))
        .stroke(Stroke::NONE)
        .min_size(vec2(0.0, size::BUTTON_PRIMARY)),
    )
}

/// A section heading with a hairline under it.
fn heading(ui: &mut Ui, theme: Theme, title: &str) {
    let t = colors(theme);
    ui.label(RichText::new(title).font(style::title()));
    ui.add_space(space::S2);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c(t.hairline));
    ui.add_space(space::S3);
}

/// One setting: name and control on the first line, description under it.
fn row(ui: &mut Ui, theme: Theme, name: &str, description: &str, control: impl FnOnce(&mut Ui)) {
    let t = colors(theme);
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).font(style::body_strong()));
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
    ui.label(RichText::new(description).color(c(t.text_secondary)));
    ui.add_space(space::S4);
}

/// An on/off switch: 36 x 20, the accent when on.
fn toggle(ui: &mut Ui, theme: Theme, on: &mut bool, label: &str) -> egui::Response {
    let t = colors(theme);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *on, label));
    let k = ui.ctx().animate_bool_responsive(resp.id, *on);
    let (fill, stroke) = if *on {
        (c(t.accent), c(t.accent))
    } else {
        (c(t.bg_raised), c(t.border_control))
    };
    ui.painter().rect(
        rect,
        10.0,
        fill,
        Stroke::new(1.0, stroke),
        egui::StrokeKind::Inside,
    );
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, k);
    let knob = if *on {
        c(t.on_accent)
    } else {
        c(t.border_control)
    };
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 7.0, knob);
    if resp.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(size::FOCUS_OFFSET),
            12.0,
            Stroke::new(size::FOCUS_RING, c(t.accent)),
            egui::StrokeKind::Outside,
        );
    }
    resp
}

/// A row of mutually exclusive choices; the chosen one is filled.
fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: Theme,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let t = colors(theme);
    let mut changed = false;
    ui.spacing_mut().item_spacing.x = space::S1;
    for (v, label) in options.iter().rev() {
        let on = *value == *v;
        let text = RichText::new(*label).color(if on { c(t.on_accent) } else { c(t.text) });
        let mut b = egui::Button::new(text).min_size(vec2(0.0, size::CONTROL));
        if on {
            b = b.fill(c(t.accent)).stroke(Stroke::NONE);
        }
        if ui.add(b).clicked() && !on {
            *value = *v;
            changed = true;
        }
    }
    changed
}

fn page(
    ui: &mut Ui,
    theme: Theme,
    section: Section,
    s: &mut Settings,
    actions: &mut Vec<UiAction>,
) {
    let before = s.clone();
    heading(ui, theme, section.name());
    match section {
        Section::Schedule => {
            row(
                ui,
                theme,
                "Change wallpaper every",
                "A new scene on this interval.",
                |ui| {
                    let current = INTERVALS
                        .iter()
                        .find(|(m, _)| *m == s.schedule.change_every_minutes)
                        .map(|(_, l)| *l)
                        .unwrap_or("Custom");
                    egui::ComboBox::from_id_salt("interval")
                        .selected_text(current)
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for (m, label) in INTERVALS {
                                ui.selectable_value(&mut s.schedule.change_every_minutes, m, label);
                            }
                        });
                },
            );
            row(
                ui,
                theme,
                "Also change at login",
                "Start each session with a fresh scene.",
                |ui| {
                    toggle(
                        ui,
                        theme,
                        &mut s.schedule.change_at_login,
                        "Also change at login",
                    );
                },
            );
            row(
                ui,
                theme,
                "Pause",
                "Keep the current wallpaper until you resume.",
                |ui| {
                    toggle(ui, theme, &mut s.schedule.paused, "Pause");
                },
            );
        }
        Section::Theme => {
            row(
                ui,
                theme,
                "Appearance",
                "Light and dark scenes use different times of day and palettes.",
                |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.theme.mode,
                        &[
                            (ThemeMode::FollowSystem, "Follow system"),
                            (ThemeMode::Light, "Light"),
                            (ThemeMode::Dark, "Dark"),
                        ],
                    );
                },
            );
        }
        Section::Buddy => {
            row(
                ui,
                theme,
                "Show Clawd",
                "The pixel-art character in each scene.",
                |ui| {
                    toggle(ui, theme, &mut s.buddy.show, "Show Clawd");
                },
            );
        }
        Section::Archive => {
            row(
                ui,
                theme,
                "Keep wallpapers for",
                "Older ones are deleted unless pinned.",
                |ui| {
                    let label = match s.archive.keep_days {
                        0 => "Forever".to_string(),
                        1 => "1 day".to_string(),
                        d => format!("{d} days"),
                    };
                    egui::ComboBox::from_id_salt("keep")
                        .selected_text(label)
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for (d, l) in [
                                (1, "1 day"),
                                (7, "7 days"),
                                (30, "30 days"),
                                (90, "90 days"),
                                (0, "Forever"),
                            ] {
                                ui.selectable_value(&mut s.archive.keep_days, d, l);
                            }
                        });
                },
            );
            row(
                ui,
                theme,
                "Archive folder",
                "Every wallpaper set, with its recipe.",
                |ui| {
                    if ui.button("Open").clicked() {
                        actions.push(UiAction::OpenArchive);
                    }
                },
            );
        }
        other => {
            let t = colors(theme);
            ui.label(
                RichText::new(format!(
                    "{} settings arrive with the next part of the app. Until then they keep their defaults.",
                    other.name()
                ))
                .color(c(t.text_secondary)),
            );
        }
    }
    if *s != before {
        actions.push(UiAction::SettingsChanged);
    }
}
