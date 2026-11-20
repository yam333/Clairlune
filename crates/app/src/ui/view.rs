//! The Settings window's content: a sidebar of sections (PLAN.md section 8),
//! the current wallpaper as a status strip above every page, and rows of
//! name, one-line description and control (docs/design.md sections 3-5).

use super::style::{self, c, colors};
use crate::settings::{Settings, ThemeMode};
use crate::tokens::{radius, size, space};
use clairlune_scene::Theme;
use egui::{Align, Align2, Layout, Rect, RichText, Sense, Stroke, Ui, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Rotation,
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
    pub const ALL: [Section; 10] = [
        Section::Rotation,
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
            Section::Rotation => "Rotation",
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

/// What the app shows about the wallpaper on screen and its own state.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// e.g. "stroll in cave".
    pub what: String,
    pub seed: Option<u64>,
    pub theme: Option<Theme>,
    /// Minutes until the next scheduled change.
    pub next_in_minutes: Option<u64>,
    pub busy: bool,
    /// Registered to start at login; `None` where this build can't manage it.
    pub start_at_login: Option<bool>,
    /// A one-line result to show under the status (export done, an error).
    pub note: Option<String>,
    /// Timeline mode: the slot boundary to come ("Next slot at 19:30").
    pub timeline_next: Option<String>,
    /// Problems in timeline.toml (missing items, unknown ids).
    pub timeline_problems: Vec<String>,
    /// What the live wallpaper does, or why it can't run.
    pub live: Option<String>,
    /// Why Live can't be chosen in this build (`None` where it can).
    pub live_supported: Option<String>,
}

/// Requests from the window to the app.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    SettingsChanged,
    NewWallpaper,
    Previous,
    OpenArchive,
    OpenLogs,
    SavePng,
    SaveGif,
    SetStartAtLogin(bool),
    RestoreWallpaper,
    SaveToLibrary,
    OpenLibrary,
    EditTimeline,
    /// The Timeline window changed the timeline (already in the model).
    TimelineChanged,
    ApplyLibrary(String),
    UpdateLibrary(Box<crate::library::Item>),
    DeleteLibrary {
        id: String,
        /// What the timeline does with slots that show it.
        forget: crate::timeline::Forget,
    },
    ExportLibrary {
        id: String,
        gif: bool,
    },
}

pub struct ViewState {
    pub section: Section,
    pub thumb: Option<egui::TextureHandle>,
    /// The moon mark for empty states.
    pub moon: Option<egui::TextureHandle>,
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

pub(super) const LANDSCAPES: [(&str, &str); 9] = [
    ("canyon", "Canyon"),
    ("mountains", "Mountains"),
    ("forest", "Forest"),
    ("desert", "Desert"),
    ("snowfield", "Snowfield"),
    ("shore", "Shore"),
    ("city_rooftops", "City rooftops"),
    ("night_sky", "Night sky"),
    ("cave", "Cave"),
];

pub(super) const ACTIVITIES: [(&str, &str); 7] = [
    ("warm_hands", "Warming hands by a fire"),
    ("stroll", "Strolling"),
    ("stargaze", "Stargazing (night only)"),
    ("wizard_magic", "Casting spells"),
    ("make_coffee", "Making coffee"),
    ("read_lantern", "Reading by a lantern"),
    ("play_guitar", "Playing guitar"),
];

/// Weather that shows as particles. Clear weather ("none", "stars") is
/// always allowed so every scene has something to fall back to.
pub(super) const WEATHER: [(&str, &str); 5] = [
    ("snow", "Snow"),
    ("rain", "Rain"),
    ("fireflies", "Fireflies"),
    ("leaves", "Falling leaves"),
    ("dust", "Dust"),
];
pub(super) const CLEAR_WEATHER: [&str; 2] = ["none", "stars"];

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
                page(ui, theme, st.section, settings, status, actions);
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
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, s.name()));
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
            focus_ring(ui, theme, rect, radius::CONTROL);
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

pub(super) fn focus_ring(ui: &Ui, theme: Theme, rect: Rect, r: f32) {
    ui.painter().rect_stroke(
        rect.expand(size::FOCUS_OFFSET),
        r + size::FOCUS_OFFSET,
        Stroke::new(size::FOCUS_RING, c(colors(theme).accent)),
        egui::StrokeKind::Outside,
    );
}

/// The wallpaper on screen, its seed and theme, the next change, and the two
/// actions people reach for most. With no wallpaper yet: the moon mark.
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
                if let Some(moon) = &st.moon {
                    let m = Rect::from_center_size(rect.center(), vec2(48.0, 48.0));
                    // Ivory reads on warm black; on paper it needs ink.
                    let tint = match theme {
                        Theme::Dark => egui::Color32::WHITE,
                        Theme::Light => c(t.text_secondary),
                    };
                    egui::Image::new(moon).tint(tint).paint_at(ui, m);
                }
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
                // Timeline mode: the slots decide.
                None if status.timeline_next.is_some() => {
                    status.timeline_next.clone().unwrap_or_default()
                }
                Some(0) => "Next change in under a minute".to_string(),
                Some(1) => "Next change in 1 minute".to_string(),
                Some(m) if m < 120 => format!("Next change in {m} minutes"),
                Some(m) => format!("Next change in {} hours", m / 60),
                None => "Changes only on demand".to_string(),
            };
            ui.label(RichText::new(next).color(c(t.text_secondary)));
            ui.add_space(space::S3);
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(!status.busy, |ui| {
                    if primary_button(ui, theme, "New wallpaper").clicked() {
                        actions.push(UiAction::NewWallpaper);
                    }
                    if ui.button("Previous").clicked() {
                        actions.push(UiAction::Previous);
                    }
                    if ui.button("Save to library").clicked() {
                        actions.push(UiAction::SaveToLibrary);
                    }
                });
            });
            if let Some(note) = &status.note {
                ui.add_space(space::S1);
                ui.label(RichText::new(note).color(c(t.text_secondary)));
            }
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

pub(super) fn primary_button(ui: &mut Ui, theme: Theme, label: &str) -> egui::Response {
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
pub(super) fn heading(ui: &mut Ui, theme: Theme, title: &str) {
    let t = colors(theme);
    ui.label(RichText::new(title).font(style::title()));
    ui.add_space(space::S2);
    hairline(ui, t.hairline);
    ui.add_space(space::S3);
}

pub(super) fn hairline(ui: &mut Ui, color: crate::tokens::Rgb) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c(color));
}

/// A group label inside a page.
pub(super) fn group(ui: &mut Ui, title: &str) {
    ui.add_space(space::S2);
    ui.label(RichText::new(title).font(style::section()));
    ui.add_space(space::S1);
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
    let enabled = ui.is_enabled();
    let (fill, stroke) = if *on {
        (c(t.accent), c(t.accent))
    } else {
        (c(t.bg_raised), c(t.border_control))
    };
    let fade = |col: egui::Color32| {
        if enabled {
            col
        } else {
            col.gamma_multiply(0.4)
        }
    };
    ui.painter().rect(
        rect,
        10.0,
        fade(fill),
        Stroke::new(1.0, fade(stroke)),
        egui::StrokeKind::Inside,
    );
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, k);
    let knob = if *on {
        c(t.on_accent)
    } else {
        c(t.border_control)
    };
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 7.0, fade(knob));
    if resp.has_focus() {
        focus_ring(ui, theme, rect, 10.0);
    }
    resp
}

/// A row of mutually exclusive choices; the chosen one is filled. Options
/// marked `false` are shown but can't be chosen.
pub(super) fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: Theme,
    value: &mut T,
    options: &[(T, &str, bool)],
) -> bool {
    let t = colors(theme);
    let mut changed = false;
    ui.spacing_mut().item_spacing.x = space::S1;
    // Right-to-left rows (controls on the right of a setting) add from the
    // right, so the options go in reversed to read left to right.
    let ordered: Vec<_> = if ui.layout().prefer_right_to_left() {
        options.iter().rev().collect()
    } else {
        options.iter().collect()
    };
    for (v, label, enabled) in ordered {
        let on = *value == *v;
        let text = RichText::new(*label).color(if on { c(t.on_accent) } else { c(t.text) });
        let mut b = egui::Button::new(text).min_size(vec2(0.0, size::CONTROL));
        if on {
            b = b.fill(c(t.accent)).stroke(Stroke::NONE);
        }
        if ui.add_enabled(*enabled, b).clicked() && !on {
            *value = *v;
            changed = true;
        }
    }
    changed
}

/// A list of checkboxes over a filter list where empty means "all". At
/// least one stays checked.
pub(super) fn filter_checks(
    ui: &mut Ui,
    list: &mut Vec<String>,
    items: &[(&str, &str)],
    always: &[&str],
) {
    let all: Vec<String> = items.iter().map(|(id, _)| id.to_string()).collect();
    let mut on: Vec<bool> = items
        .iter()
        .map(|(id, _)| list.is_empty() || list.iter().any(|x| x == id))
        .collect();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::S4, space::S2);
        for (i, (_, label)) in items.iter().enumerate() {
            let last_one = on[i] && on.iter().filter(|x| **x).count() == 1;
            if ui
                .add_enabled(!last_one, egui::Checkbox::new(&mut on[i], *label))
                .changed()
            {
                changed = true;
            }
        }
    });
    if changed {
        let picked: Vec<String> = all
            .iter()
            .zip(&on)
            .filter(|(_, o)| **o)
            .map(|(id, _)| id.clone())
            .collect();
        *list = if picked.len() == all.len() {
            Vec::new()
        } else {
            always.iter().map(|s| s.to_string()).chain(picked).collect()
        };
    }
}

fn page(
    ui: &mut Ui,
    theme: Theme,
    section: Section,
    s: &mut Settings,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let before = s.clone();
    heading(ui, theme, section.name());
    match section {
        Section::Rotation => {
            row(
                ui,
                theme,
                "Mode",
                "Generate makes a new scene on the schedule. Timeline follows a daily plan of saved wallpapers and random scenes.",
                |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.rotation.mode,
                        &[
                            (crate::settings::RotationMode::Generate, "Generate", true),
                            (crate::settings::RotationMode::Timeline, "Timeline", true),
                        ],
                    );
                },
            );
            row(
                ui,
                theme,
                "Timeline",
                "The daily plan: which wallpaper shows when, for every day or each weekday.",
                |ui| {
                    if ui.button("Edit timeline").clicked() {
                        actions.push(UiAction::EditTimeline);
                    }
                },
            );
            if s.rotation.mode == crate::settings::RotationMode::Timeline {
                if let Some(next) = &status.timeline_next {
                    ui.label(
                        RichText::new(next)
                            .font(style::mono())
                            .color(c(t.text_secondary)),
                    );
                }
                for p in &status.timeline_problems {
                    ui.label(RichText::new(p).color(c(t.danger)));
                }
            }
        }
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
                "Light and dark scenes use different times of day and palettes. Night-only scenes stay dark.",
                |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.theme.mode,
                        &[
                            (ThemeMode::FollowSystem, "Follow system", true),
                            (ThemeMode::Light, "Light", true),
                            (ThemeMode::Dark, "Dark", true),
                        ],
                    );
                },
            );
        }
        Section::Display => {
            use crate::settings::WallpaperMode;
            let can_live = status.live_supported.is_none();
            row(
                ui,
                theme,
                "Wallpaper mode",
                "Live plays the scene behind your desktop icons: Clawd moves, fires flicker, snow falls. Still frame sets one image and does nothing between changes.",
                |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.display.mode,
                        &[
                            (WallpaperMode::Still, "Still frame", true),
                            (WallpaperMode::Live, "Live", can_live),
                        ],
                    );
                },
            );
            if let Some(why) = &status.live_supported {
                ui.label(RichText::new(why).color(c(t.text_secondary)));
                ui.add_space(space::S4);
            } else if s.display.mode == WallpaperMode::Live
                && let Some(live) = &status.live
            {
                ui.label(
                    RichText::new(live)
                        .font(style::mono())
                        .color(c(t.text_secondary)),
                );
                ui.add_space(space::S4);
            }
            row(
                ui,
                theme,
                "Monitors",
                "Each monitor gets its own scene at its own resolution, all from one seed.",
                |_| {},
            );
        }
        Section::Live => {
            const SMOOTHNESS: [(u32, &str, &str); 5] = [
                (
                    6,
                    "Battery saver",
                    "6 fps. Lowest power; Clawd still moves, more choppily.",
                ),
                (
                    12,
                    "Balanced",
                    "12 fps. The pace the pixel art is drawn at.",
                ),
                (24, "Smooth", "24 fps. Smoother snow, sparks and drifting."),
                (30, "Very smooth", "30 fps."),
                (60, "Ultra smooth", "60 fps. Highest power."),
            ];
            let current = SMOOTHNESS
                .iter()
                .find(|(f, _, _)| *f == s.live.smoothness_fps)
                .copied()
                .unwrap_or(SMOOTHNESS[1]);
            row(ui, theme, "Smoothness", current.2, |ui| {
                egui::ComboBox::from_id_salt("smoothness")
                    .selected_text(current.1)
                    .show_ui(ui, |ui| {
                        for (fps, name, _) in SMOOTHNESS {
                            ui.selectable_value(&mut s.live.smoothness_fps, fps, name);
                        }
                    });
            });
            ui.label(
                RichText::new(
                    "A monitor covered by a maximized or full-screen app pauses until it shows again. Frames are drawn on the graphics card the monitor is plugged into.",
                )
                .color(c(t.text_secondary)),
            );
            if s.display.mode != crate::settings::WallpaperMode::Live {
                ui.add_space(space::S2);
                ui.label(
                    RichText::new("The wallpaper is a still frame now (Display, Wallpaper mode).")
                        .color(c(t.text_secondary)),
                );
            }
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
            ui.label(
                RichText::new("Clawd is Anthropic's character, drawn here as fan art. Turning him off keeps the scenes and their props.")
                    .color(c(t.text_secondary)),
            );
        }
        Section::Content => {
            ui.label(
                RichText::new(
                    "New scenes pick from what's checked. Changes apply to the next wallpaper.",
                )
                .color(c(t.text_secondary)),
            );
            group(ui, "Landscapes");
            filter_checks(ui, &mut s.content.archetypes, &LANDSCAPES, &[]);
            ui.add_space(space::S4);
            group(ui, "What Clawd does");
            filter_checks(ui, &mut s.content.activities, &ACTIVITIES, &[]);
            ui.add_space(space::S4);
            group(ui, "Weather");
            filter_checks(ui, &mut s.content.weather, &WEATHER, &CLEAR_WEATHER);
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
                "Size limit",
                "The oldest wallpapers go first when the archive grows past this.",
                |ui| {
                    egui::ComboBox::from_id_salt("cap")
                        .selected_text(format!("{} GB", s.archive.max_mb.div_ceil(1024).max(1)))
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for gb in [1u64, 2, 5, 10] {
                                ui.selectable_value(
                                    &mut s.archive.max_mb,
                                    gb * 1024,
                                    format!("{gb} GB"),
                                );
                            }
                        });
                },
            );
            row(
                ui,
                theme,
                "Library",
                "Wallpapers you saved. They are never deleted automatically.",
                |ui| {
                    if ui.button("Open").clicked() {
                        actions.push(UiAction::OpenLibrary);
                    }
                },
            );
            row(
                ui,
                theme,
                "Archive folder",
                "Every wallpaper set, with its recipe and images.",
                |ui| {
                    if ui.button("Open").clicked() {
                        actions.push(UiAction::OpenArchive);
                    }
                },
            );
        }
        Section::Export => {
            row(
                ui,
                theme,
                "Save current as PNG",
                "Copies the primary monitor's wallpaper to the export folder.",
                |ui| {
                    if ui.button("Save").clicked() {
                        actions.push(UiAction::SavePng);
                    }
                },
            );
            row(
                ui,
                theme,
                "Save current as GIF",
                "A 6-second loop of the scene as it plays, for sharing. Saved wallpapers keep the full scene.",
                |ui| {
                    if ui.button("Save").clicked() {
                        actions.push(UiAction::SaveGif);
                    }
                },
            );
            ui.label(RichText::new("Export folder").font(style::body_strong()));
            let hint = "Empty: Pictures\\Clairlune";
            ui.add(
                egui::TextEdit::singleline(&mut s.export.folder)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            ui.label(RichText::new("Where saved images go.").color(c(t.text_secondary)));
        }
        Section::General => {
            match status.start_at_login {
                Some(mut on) => row(
                    ui,
                    theme,
                    "Start at login",
                    "Run Clairlune in the tray when you sign in.",
                    |ui| {
                        if toggle(ui, theme, &mut on, "Start at login").changed() {
                            actions.push(UiAction::SetStartAtLogin(on));
                        }
                    },
                ),
                None => row(
                    ui,
                    theme,
                    "Start at login",
                    "Available with the installers for this system.",
                    |ui| {
                        ui.add_enabled_ui(false, |ui| {
                            let mut off = false;
                            toggle(ui, theme, &mut off, "Start at login");
                        });
                    },
                ),
            }
            row(
                ui,
                theme,
                "Restore my wallpaper",
                "Puts back the wallpaper you had before Clairlune and pauses the schedule.",
                |ui| {
                    if ui.button("Restore").clicked() {
                        actions.push(UiAction::RestoreWallpaper);
                    }
                },
            );
            row(ui, theme, "Logs", "Details for bug reports.", |ui| {
                if ui.button("Open").clicked() {
                    actions.push(UiAction::OpenLogs);
                }
            });
            ui.add_space(space::S2);
            hairline(ui, t.hairline);
            ui.add_space(space::S3);
            ui.label(
                RichText::new(format!(
                    "Clairlune {}  generator {}",
                    env!("CARGO_PKG_VERSION"),
                    clairlune_scene::GENERATOR_VERSION
                ))
                .font(style::mono())
                .color(c(t.text_secondary)),
            );
            ui.label(
                RichText::new(
                    "Unofficial fan project, not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character. Code under MIT or Apache-2.0; fonts under the SIL Open Font License.",
                )
                .color(c(t.text_secondary)),
            );
        }
    }
    if *s != before {
        actions.push(UiAction::SettingsChanged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_lists_cover_the_real_ids() {
        for (id, _) in LANDSCAPES {
            assert!(clairlune_scene::archetype::archetype(id).is_some(), "{id}");
        }
        let catalog = clairlune_scene::activity::builtin_catalog();
        for (id, _) in ACTIVITIES {
            assert!(catalog.get(id).is_some(), "{id}");
        }
        assert_eq!(
            ACTIVITIES.len(),
            catalog.activities.len(),
            "a new activity needs a label"
        );
        for id in WEATHER.iter().map(|(id, _)| *id).chain(CLEAR_WEATHER) {
            assert!(clairlune_scene::archetype::WEATHERS.contains(&id), "{id}");
        }
    }
}
