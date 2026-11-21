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
        Section::Rotation,
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
    /// Ask, once, whether to start quietly at sign-in (the first run).
    pub ask_start_at_login: bool,
    /// macOS: this copy carries the screen saver (from the DMG or Homebrew).
    pub saver_available: bool,
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
    /// How many crash reports there are.
    pub crash_reports: usize,
    /// Detailed logging is on until this local time ("18:40 tomorrow").
    pub detailed_until: Option<String>,
}

/// Requests from the window to the app.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    SettingsChanged,
    NewWallpaper,
    Previous,
    OpenArchive,
    OpenLogs,
    OpenCrashReports,
    ExportDiagnostics,
    SetDetailedLogging(bool),
    SavePng,
    SaveGif,
    SetStartAtLogin(bool),
    /// The first-run question's answer: start quietly at sign-in or not.
    AnswerStartAtLogin(bool),
    Quit,
    RestoreWallpaper,
    SaveToLibrary,
    OpenLibrary,
    EditTimeline,
    OpenArchiveFolder,
    ApplyArchive(String),
    PinArchive {
        name: String,
        pinned: bool,
    },
    SaveArchiveToLibrary(String),
    ExportArchive {
        name: String,
        gif: bool,
    },
    DeleteArchive(String),
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
    /// A brief note over the page and when it appeared.
    pub toast: Option<(String, std::time::Instant)>,
}

/// How long a brief note stays.
const TOAST: std::time::Duration = std::time::Duration::from_secs(2);

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

/// Filter items: id, short label, and what limits it (shown on hover).
pub(super) type Item = (&'static str, &'static str, &'static str);

/// The looks, by their ids in the recipe.
pub(super) const LOOKS: [Item; 2] = [
    ("line", "Line art", "The land in strokes of text"),
    (
        "dither",
        "Dithered",
        "Light and dark masses of dots and bars",
    ),
];

/// The zooms, by their ids in the recipe.
pub(super) const ZOOMS: [Item; 3] = [
    ("close", "Close", "Clawd up close, as he has always been"),
    (
        "wide",
        "Wide",
        "Clawd about half the size, in a bigger landscape",
    ),
    ("vast", "Vast", "Clawd tiny in a huge landscape"),
];

pub(super) const LANDSCAPES: [Item; 10] = [
    ("canyon", "Canyon", ""),
    ("mountains", "Mountains", ""),
    ("forest", "Forest", ""),
    ("desert", "Desert", ""),
    ("snowfield", "Snowfield", ""),
    ("shore", "Shore", ""),
    ("city_rooftops", "Rooftops", "City rooftops"),
    ("night_sky", "Night sky", ""),
    ("cave", "Cave", ""),
    (
        "interior",
        "Indoors",
        "A room, with a landscape through the window",
    ),
];

pub(super) const ACTIVITIES: [Item; 26] = [
    ("warm_hands", "Campfire", "Warming hands by a fire"),
    ("stroll", "Strolling", ""),
    ("stargaze", "Stargazing", "Night only"),
    ("wizard_magic", "Spells", "Casting spells"),
    ("make_coffee", "Coffee", "Making coffee"),
    ("read_lantern", "Reading", "Reading by a lantern"),
    ("play_guitar", "Guitar", "Playing guitar"),
    ("fishing", "Fishing", "Off a pier, shore only"),
    ("fly_kite", "Kite", "Flying a kite, daytime"),
    ("skip_stones", "Skipping stones", "Shore only"),
    ("broom_flight", "Broom flight", "Evening and night"),
    ("paint_easel", "Painting", "At an easel, daytime"),
    ("snowballs", "Snowballs", "Snow only"),
    ("balloon_ride", "Balloon ride", "A hot-air balloon, daytime"),
    ("picnic", "Picnic", "Daytime"),
    ("chess", "Chess", ""),
    ("tennis", "Tennis", "Against a wall, daytime"),
    ("rowing", "Rowing", "A boat, shore only"),
    ("skiing", "Skiing", "Snow only"),
    (
        "hike_ridge",
        "Ridge hike",
        "Mountains; the land scrolls past",
    ),
    ("ski_tour", "Ski touring", "Snow; the land scrolls past"),
    ("sledding", "Sledding", "Snow; the land scrolls past"),
    (
        "forest_walk",
        "Forest walk",
        "Forest; the land scrolls past",
    ),
    (
        "desert_trek",
        "Desert trek",
        "Desert; the land scrolls past",
    ),
    (
        "broom_journey",
        "Broom journey",
        "Evening and night; the land scrolls past below",
    ),
    (
        "balloon_journey",
        "Balloon journey",
        "Daytime; the land drifts past below",
    ),
];

/// Weather that shows as particles. Clear weather ("none", "stars") is
/// always allowed so every scene has something to fall back to.
pub(super) const WEATHER: [Item; 5] = [
    ("snow", "Snow", ""),
    ("rain", "Rain", ""),
    ("fireflies", "Fireflies", ""),
    ("leaves", "Leaves", "Falling leaves"),
    ("dust", "Dust", ""),
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
                let buddy = settings.buddy.show;
                page(ui, theme, st.section, settings, status, actions);
                if settings.buddy.show != buddy {
                    let text = if settings.buddy.show {
                        "Clawd joins the next wallpaper"
                    } else {
                        "Clawd skips the next wallpaper"
                    };
                    st.toast = Some((text.to_string(), std::time::Instant::now()));
                }
            });
        });
    toast(ui, theme, st);
    if status.ask_start_at_login {
        ask_start_at_login(ui, theme, actions);
    }
}

/// Where the app lives when no window is open: the tray, or the menu bar.
fn tray_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "menu bar"
    } else {
        "tray"
    }
}

/// The first run's question, over the window: start quietly at sign-in?
/// Escape or a click outside is "Not now".
fn ask_start_at_login(ui: &mut Ui, theme: Theme, actions: &mut Vec<UiAction>) {
    let t = colors(theme);
    let modal = egui::Modal::new(egui::Id::unique("ask-start-at-login"))
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_raised))
                .stroke(Stroke::new(size::HAIRLINE, c(t.hairline)))
                .corner_radius(radius::PANEL)
                .inner_margin(egui::Margin::same(space::S5 as i8)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_max_width(380.0);
            ui.label(RichText::new("Start Clairlune with your computer?").font(style::section()));
            ui.add_space(space::S2);
            ui.label(
                RichText::new(format!(
                    "It starts quietly when you sign in: no window, just the {} icon, and the wallpaper changes on its own. You can change this later under General.",
                    tray_name()
                ))
                .color(c(t.text_secondary)),
            );
            ui.add_space(space::S4);
            ui.horizontal(|ui| {
                if primary_button(ui, theme, "Start at sign-in").clicked() {
                    actions.push(UiAction::AnswerStartAtLogin(true));
                }
                ui.add_space(space::S2);
                if ui.button("Not now").clicked() {
                    actions.push(UiAction::AnswerStartAtLogin(false));
                }
            });
        });
    if modal.should_close()
        && !actions
            .iter()
            .any(|a| matches!(a, UiAction::AnswerStartAtLogin(_)))
    {
        actions.push(UiAction::AnswerStartAtLogin(false));
    }
}

/// A faint note at the bottom of the window that fades after two seconds.
fn toast(ui: &mut Ui, theme: Theme, st: &mut ViewState) {
    let Some((text, since)) = &st.toast else {
        return;
    };
    let age = since.elapsed();
    if age >= TOAST {
        st.toast = None;
        return;
    }
    // Fade in quickly and out over the last half second.
    let a = age.as_secs_f32();
    let left = TOAST.as_secs_f32() - a;
    let alpha = (a / 0.15).min(left / 0.5).clamp(0.0, 1.0);
    let t = colors(theme);
    egui::Area::new(egui::Id::unique("toast"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -space::S6))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(c(t.bg_raised).gamma_multiply(alpha))
                .stroke(Stroke::new(
                    size::HAIRLINE,
                    c(t.hairline).gamma_multiply(alpha),
                ))
                .corner_radius(radius::PANEL)
                .inner_margin(egui::Margin::symmetric(space::S4 as i8, space::S2 as i8))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).color(c(t.text_secondary).gamma_multiply(alpha)));
                });
        });
    ui.ctx().request_repaint();
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
/// The Theme page's Schedule settings: two times of day, or sunset and
/// sunrise at a place (worked out on this computer).
fn theme_schedule(ui: &mut Ui, theme: Theme, s: &mut Settings) {
    let t = colors(theme);
    let note = |ui: &mut Ui, text: &str| {
        ui.label(RichText::new(text).color(c(t.text_secondary)));
        ui.add_space(space::S4);
    };
    row(
        ui,
        theme,
        "Follow the sun",
        "Dark from sunset to sunrise where you are. Worked out on this computer; nothing goes online.",
        |ui| {
            toggle(ui, theme, &mut s.theme.follow_sun, "Follow the sun");
        },
    );
    if s.theme.follow_sun {
        ui.label(RichText::new("Location").font(style::body_strong()));
        let location = settled_text(
            ui,
            "theme-location",
            &mut s.theme.location,
            "40.71, -74.01",
            220.0,
            |t| t.trim().is_empty() || crate::sun::parse_location(t).is_some(),
        );
        match crate::sun::parse_location(&location) {
            Some((lat, lon)) => {
                let today = jiff::ZonedDateTime::now().date();
                let text = match crate::sun::daylight(today, lat, lon) {
                    crate::sun::Daylight::Normal { rise, set } => format!(
                        "Today: sunrise {}, sunset {}.",
                        crate::timeline::local_hhmm(rise),
                        crate::timeline::local_hhmm(set)
                    ),
                    crate::sun::Daylight::AlwaysUp => "The sun stays up all day today.".into(),
                    crate::sun::Daylight::AlwaysDown => "The sun stays down all day today.".into(),
                };
                note(ui, &text);
            }
            None => note(
                ui,
                "Latitude and longitude in degrees, south and west negative: 40.71, -74.01 for New York.",
            ),
        }
        return;
    }
    let mut valid = true;
    row(
        ui,
        theme,
        "Dark between",
        "Light the rest of the day. 24-hour times.",
        |ui| {
            let ok = |text: &str| crate::settings::parse_clock(text).is_some();
            // Right to left: the second time first.
            let light = settled_text(
                ui,
                "theme-light-from",
                &mut s.theme.light_from,
                "",
                56.0,
                ok,
            );
            ui.label("and");
            let dark = settled_text(ui, "theme-dark-from", &mut s.theme.dark_from, "", 56.0, ok);
            valid = ok(&dark) && ok(&light);
        },
    );
    if !valid {
        note(ui, "Use 24-hour times like 19:00 and 07:00.");
    }
}

/// A text field whose text reaches the settings once it's done (focus
/// leaves, or Enter) and `valid` takes it: a half-typed value doesn't
/// change the settings, and the wallpaper, on every key. Returns the text
/// shown, which may still be wrong.
fn settled_text(
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
        let color = if on {
            t.on_accent
        } else if *enabled {
            t.text
        } else {
            t.text_disabled
        };
        let text = RichText::new(*label).color(c(color));
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

/// A filter list where empty means "all", as a row of chips: a filled
/// pill when on, a quiet outline when off, what limits each in its
/// tooltip. At least one stays on; "All" turns every one back on.
pub(super) fn filter_checks(ui: &mut Ui, list: &mut Vec<String>, items: &[Item], always: &[&str]) {
    let all: Vec<String> = items.iter().map(|(id, _, _)| id.to_string()).collect();
    let mut on: Vec<bool> = items
        .iter()
        .map(|(id, _, _)| list.is_empty() || list.iter().any(|x| x == id))
        .collect();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
        for (i, (_, label, hint)) in items.iter().enumerate() {
            let last_one = on[i] && on.iter().filter(|x| **x).count() == 1;
            let mut r = chip(ui, on[i], label, !last_one);
            if !hint.is_empty() {
                r = r.on_hover_text(*hint);
            }
            if r.clicked() {
                on[i] = !on[i];
                changed = true;
            }
        }
        // Back to everything, when something is off.
        if on.iter().any(|o| !o) {
            let link = ui.add(
                egui::Label::new(
                    RichText::new("All")
                        .color(ui.visuals().hyperlink_color)
                        .font(style::body()),
                )
                .sense(egui::Sense::click()),
            );
            if link
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                on.fill(true);
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

/// One filter chip: a rounded pill, filled with a tint of the accent and
/// a check when on, outlined when off. `enabled` false: it can't be
/// turned off (the last one on).
fn chip(ui: &mut Ui, on: bool, label: &str, enabled: bool) -> egui::Response {
    let v = ui.visuals().clone();
    let accent = v.selection.bg_fill;
    let text = if on {
        v.text_color()
    } else {
        v.widgets.noninteractive.fg_stroke.color
    };
    let font = style::body();
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, text);
    let check_w = if on { 14.0 } else { 0.0 };
    let pad = space::S3;
    let h = 26.0;
    let w = galley.size().x + 2.0 * pad + check_w;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let radius = egui::CornerRadius::same((h / 2.0) as u8);
        let hovered = resp.hovered() && enabled;
        let (fill, stroke) = if on {
            (
                accent.gamma_multiply(if hovered { 0.28 } else { 0.18 }),
                Stroke::new(1.0, accent.gamma_multiply(0.9)),
            )
        } else {
            (
                if hovered {
                    v.widgets.hovered.weak_bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                },
                Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color),
            )
        };
        p.rect(rect, radius, fill, stroke, egui::StrokeKind::Inside);
        let mut x = rect.left() + pad;
        if on {
            // A small check mark, drawn (no font has to have one).
            let cy = rect.center().y;
            let pts = [
                egui::pos2(x, cy),
                egui::pos2(x + 3.5, cy + 3.5),
                egui::pos2(x + 9.0, cy - 3.5),
            ];
            p.line(pts.to_vec(), Stroke::new(1.6, accent));
            x += check_w;
        }
        let y = rect.center().y - galley.size().y / 2.0;
        p.galley(egui::pos2(x, y), galley, text);
    }
    resp.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::NotAllowed
    })
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
            use crate::settings::RotationMode;
            row(
                ui,
                theme,
                "Mode",
                "Generate makes a new scene on a schedule. Timeline follows a daily plan of saved wallpapers and random scenes.",
                |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.rotation.mode,
                        &[
                            (RotationMode::Generate, "Generate", true),
                            (RotationMode::Timeline, "Timeline", true),
                        ],
                    );
                },
            );
            let timeline = s.rotation.mode == RotationMode::Timeline;
            if timeline {
                row(
                    ui,
                    theme,
                    "Timeline",
                    "Which wallpaper shows when, for every day or each weekday.",
                    |ui| {
                        if ui.button("Edit timeline").clicked() {
                            actions.push(UiAction::EditTimeline);
                        }
                    },
                );
                if let Some(next) = &status.timeline_next {
                    ui.label(
                        RichText::new(next)
                            .font(style::mono())
                            .color(c(t.text_secondary)),
                    );
                    ui.add_space(space::S2);
                }
                for p in &status.timeline_problems {
                    ui.label(RichText::new(p).color(c(t.danger)));
                }
                if !status.timeline_problems.is_empty() {
                    ui.add_space(space::S4);
                }
            }
            let (name, description) = if timeline {
                (
                    "Days without slots change every",
                    "Days with no slots get new scenes on this interval.",
                )
            } else {
                ("Change wallpaper every", "A new scene on this interval.")
            };
            row(ui, theme, name, description, |ui| {
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
            });
            if !timeline {
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
            }
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
                            (ThemeMode::Schedule, "Schedule", true),
                        ],
                    );
                },
            );
            if s.theme.mode == ThemeMode::Schedule {
                theme_schedule(ui, theme, s);
            }
            ui.add_space(space::S4);
            group(ui, "Look");
            ui.label(
                RichText::new(
                    "Line art draws the land in strokes of text. Dithered draws it as light and dark masses, every character a dot or a bar sized by how bright it is there. Changes apply to the next wallpaper.",
                )
                .color(c(t.text_secondary)),
            );
            filter_checks(ui, &mut s.content.looks, &LOOKS, &[]);
            ui.add_space(space::S4);
            group(ui, "Zoom");
            ui.label(
                RichText::new(
                    "How far back the view stands. Close shows Clawd as he has always been; Wide and Vast make him smaller and the landscape bigger. With more than one on, each wallpaper picks one.",
                )
                .color(c(t.text_secondary)),
            );
            filter_checks(ui, &mut s.content.zooms, &ZOOMS, &[]);
            ui.add_space(space::S2);
            row(
                ui,
                theme,
                "Contrast",
                "How strong the ink is against the background. Vary picks one for each wallpaper.",
                |ui| {
                    use crate::settings::Contrast;
                    segmented(
                        ui,
                        theme,
                        &mut s.content.contrast,
                        &[
                            (Contrast::Soft, "Soft", true),
                            (Contrast::Normal, "Normal", true),
                            (Contrast::High, "High", true),
                            (Contrast::Vary, "Vary", true),
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
                "One scene each, at each monitor's own resolution, or one wide scene spread across all of them (best with monitors side by side).",
                |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.display.span,
                        &[(false, "One each", true), (true, "Spread across", true)],
                    );
                },
            );
        }
        Section::Live => {
            // Every setting plays at the same speed; only the choppiness and
            // the work differ.
            const SMOOTHNESS: [(u32, &str, &str); 5] = [
                (
                    6,
                    "Battery saver",
                    "6 frames a second: the lightest, and the choppiest. Every setting moves at the same speed.",
                ),
                (
                    12,
                    "Balanced",
                    "12 frames a second, the pixel art's own rhythm. Every setting moves at the same speed.",
                ),
                (
                    24,
                    "Smooth",
                    "24 frames a second: smoother, a little more work. Every setting moves at the same speed.",
                ),
                (
                    30,
                    "Very smooth",
                    "30 frames a second. Every setting moves at the same speed.",
                ),
                (
                    60,
                    "Ultra smooth",
                    "60 frames a second: the smoothest, the most work. Every setting moves at the same speed.",
                ),
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
            row(
                ui,
                theme,
                "Pause behind full-screen apps",
                "When an app is maximized or full screen, the wallpaper under it stops moving.",
                |ui| {
                    use crate::settings::CoverPause;
                    segmented(
                        ui,
                        theme,
                        &mut s.live.pause_when_covered,
                        &[
                            (CoverPause::ThatMonitor, "That monitor", true),
                            (CoverPause::AllMonitors, "All monitors", true),
                            (CoverPause::Off, "Off", true),
                        ],
                    );
                },
            );
            row(
                ui,
                theme,
                "Battery saver on battery",
                "On battery, drop to 6 frames a second.",
                |ui| {
                    toggle(
                        ui,
                        theme,
                        &mut s.live.battery_saver_on_battery,
                        "Battery saver on battery",
                    );
                },
            );
            row(
                ui,
                theme,
                "Pause on battery",
                "On battery, stop the animation; the frame stays on screen.",
                |ui| {
                    toggle(ui, theme, &mut s.live.pause_on_battery, "Pause on battery");
                },
            );
            ui.label(
                RichText::new(
                    "Live also pauses while the computer is locked or the displays are off. Frames are drawn on the graphics card the monitor is plugged into.",
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
            ui.horizontal(|ui| {
                ui.label(RichText::new("Show Clawd").font(style::body_strong()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, theme, &mut s.buddy.show, "Show Clawd");
                });
            });
        }
        Section::Content => {
            ui.label(
                RichText::new(
                    "New scenes pick from what's selected; hover for what each needs. Changes apply to the next wallpaper.",
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
            let tray_desc = format!(
                "Starts quietly in the {} when you sign in, with no window.",
                tray_name()
            );
            match status.start_at_login {
                Some(mut on) => row(ui, theme, "Start at login", &tray_desc, |ui| {
                    if toggle(ui, theme, &mut on, "Start at login").changed() {
                        actions.push(UiAction::SetStartAtLogin(on));
                    }
                }),
                None => row(
                    ui,
                    theme,
                    "Start at login",
                    "Not available on this system.",
                    |ui| {
                        ui.add_enabled_ui(false, |ui| {
                            let mut off = false;
                            toggle(ui, theme, &mut off, "Start at login");
                        });
                    },
                ),
            }
            let (icon_name, back) = if cfg!(target_os = "macos") {
                ("Show menu bar icon", "Launchpad or Spotlight")
            } else if cfg!(windows) {
                ("Show tray icon", "the Start menu")
            } else {
                ("Show tray icon", "your app menu")
            };
            row(
                ui,
                theme,
                icon_name,
                &format!(
                    "Hidden, Clairlune keeps running in the background. Open it again from {back} to come back here."
                ),
                |ui| {
                    toggle(ui, theme, &mut s.general.show_tray_icon, icon_name);
                },
            );
            if cfg!(target_os = "macos") {
                if status.saver_available {
                    row(
                        ui,
                        theme,
                        "Screen saver",
                        "Plays the wallpaper on screen as the screen saver too. Turning it on adds Clairlune to System Settings, Screen Saver, and opens it there: pick Clairlune.",
                        |ui| {
                            toggle(ui, theme, &mut s.general.screen_saver, "Screen saver");
                        },
                    );
                } else {
                    row(
                        ui,
                        theme,
                        "Screen saver",
                        "Comes with the app from the DMG or Homebrew.",
                        |ui| {
                            ui.add_enabled_ui(false, |ui| {
                                let mut off = false;
                                toggle(ui, theme, &mut off, "Screen saver");
                            });
                        },
                    );
                }
            }
            row(
                ui,
                theme,
                "Quit Clairlune",
                "Stops it until you open it again. The wallpaper on screen stays.",
                |ui| {
                    if ui.button("Quit").clicked() {
                        actions.push(UiAction::Quit);
                    }
                },
            );
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
            let detailed_text = match &status.detailed_until {
                Some(until) => format!("On until {until}, then back to normal."),
                None => "Logs much more for a day, for a bug report.".to_string(),
            };
            row(ui, theme, "Detailed logging", &detailed_text, |ui| {
                let mut on = status.detailed_until.is_some();
                if toggle(ui, theme, &mut on, "Detailed logging").changed() {
                    actions.push(UiAction::SetDetailedLogging(on));
                }
            });
            row(
                ui,
                theme,
                "Diagnostics",
                "One zip for a bug report: system facts, recent logs, crash reports, recipes and settings, with your home folder written as ~. Nothing is sent anywhere.",
                |ui| {
                    if ui.button("Export").clicked() {
                        actions.push(UiAction::ExportDiagnostics);
                    }
                },
            );
            if status.crash_reports > 0 {
                let n = status.crash_reports;
                row(
                    ui,
                    theme,
                    "Crash reports",
                    &format!(
                        "{n} report{} from times Clairlune stopped unexpectedly.",
                        if n == 1 { "" } else { "s" }
                    ),
                    |ui| {
                        if ui.button("Open").clicked() {
                            actions.push(UiAction::OpenCrashReports);
                        }
                    },
                );
            }
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

    /// Renders Settings with the first run's question up for `frames`, each
    /// with `events`, and returns the actions asked for.
    fn ask_frames(events: &[egui::Event], frames: usize) -> Vec<UiAction> {
        let ctx = egui::Context::default();
        style::apply(&ctx, Theme::Light);
        let mut st = ViewState {
            section: Section::General,
            thumb: None,
            moon: None,
            toast: None,
        };
        let mut settings = Settings::default();
        let status = Status {
            ask_start_at_login: true,
            start_at_login: Some(false),
            ..Status::default()
        };
        let mut actions = Vec::new();
        for i in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 640.0),
                )),
                events: if i + 1 == frames {
                    events.to_vec()
                } else {
                    Vec::new()
                },
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                show(
                    ui,
                    Theme::Light,
                    &mut st,
                    &mut settings,
                    &status,
                    &mut actions,
                )
            })
            .drop_without_applying_deltas();
        }
        actions
    }

    #[test]
    fn the_first_run_question_waits_and_escape_is_not_now() {
        // Up, and nothing is answered until the user does.
        assert!(ask_frames(&[], 3).is_empty());
        let escape = |pressed| egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(
            ask_frames(&[escape(true), escape(false)], 3),
            vec![UiAction::AnswerStartAtLogin(false)]
        );
    }

    #[test]
    fn a_half_typed_time_reaches_the_settings_only_when_done() {
        use egui::{Event, Key, Modifiers, RawInput};
        let ctx = egui::Context::default();
        let mut value = String::new();
        let frame = |events: Vec<Event>, value: &mut String| {
            let input = RawInput {
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                settled_text(ui, "test-clock", value, "", 56.0, |t| {
                    crate::settings::parse_clock(t).is_some()
                });
            })
            .drop_without_applying_deltas();
        };
        frame(Vec::new(), &mut value);
        ctx.memory_mut(|m| {
            m.request_focus(egui::Id::unique(("settled-text", "test-clock")).with("edit"))
        });
        frame(Vec::new(), &mut value);
        for key in ["2", "0", ":", "0", "0"] {
            frame(vec![Event::Text(key.into())], &mut value);
            assert_eq!(value, "", "changed while typing");
        }
        let enter = |pressed| Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        frame(vec![enter(true), enter(false)], &mut value);
        frame(Vec::new(), &mut value);
        assert_eq!(value, "20:00");
    }

    #[test]
    fn content_lists_cover_the_real_ids() {
        for (id, _, _) in LANDSCAPES {
            assert!(clairlune_scene::archetype::archetype(id).is_some(), "{id}");
        }
        let catalog = clairlune_scene::activity::builtin_catalog();
        for (id, _, _) in ACTIVITIES {
            assert!(catalog.get(id).is_some(), "{id}");
        }
        assert_eq!(
            ACTIVITIES.len(),
            catalog.activities.len(),
            "a new activity needs a label"
        );
        for id in WEATHER.iter().map(|(id, _, _)| *id).chain(CLEAR_WEATHER) {
            assert!(clairlune_scene::archetype::WEATHERS.contains(&id), "{id}");
        }
        let zooms: Vec<&str> = ZOOMS.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(zooms, clairlune_scene::ZOOMS);
    }
}
