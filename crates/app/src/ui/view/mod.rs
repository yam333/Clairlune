//! The Settings window's content (Moonglass, docs/dev/design.md): a sidebar of
//! sections on reeded glass with the wallpaper on screen at its foot, and
//! pages on smoked glass: a serif title, then rows of name, one-line
//! description and control.

use super::glass;
use super::style::{self, alpha, c, colors};
use crate::settings::{Settings, ThemeMode};
use crate::tokens::{radius, size, space};
use clairlune_scene::Theme;
use egui::{
    Align, Align2, Color32, Layout, Painter, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2,
};
use std::collections::HashMap;
use std::path::PathBuf;

mod arrangement;
mod catalog;
mod pages;
mod sidebar;
mod welcome;
mod widgets;
pub(crate) use arrangement::*;
pub(crate) use catalog::*;
pub(crate) use pages::*;
pub(crate) use sidebar::*;
pub(crate) use welcome::*;
pub(crate) use widgets::*;

/// Windows: Settings draws its own frame (minimize and close at the top
/// right, moved by its top edge, resized at its edges). macOS keeps its
/// own, see-through, the buttons on the glass at the top left.
pub const OWN_FRAME: bool = cfg!(windows);

/// Room at the top for macOS's window buttons.
const MAC_TOP: f32 = if cfg!(target_os = "macos") { 28.0 } else { 0.0 };

/// What the window's own frame asks of the window this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome {
    Drag,
    Resize(winit::window::ResizeDirection),
    Minimize,
    Close,
}

/// Content's landscape tiles: small scenes the app draws once (see
/// `land_thumbs` in main.rs) and loads here as they appear.
#[derive(Default)]
pub struct LandThumbs {
    pub dir: Option<PathBuf>,
    loaded: HashMap<String, Option<egui::TextureHandle>>,
}

impl LandThumbs {
    fn get(&mut self, ctx: &egui::Context, id: &str) -> Option<&egui::TextureHandle> {
        let dir = self.dir.as_ref()?;
        self.loaded
            .entry(id.to_string())
            .or_insert_with(|| {
                let bytes = std::fs::read(dir.join(format!("{id}.png"))).ok()?;
                let (size, rgba) = super::decode_png_rgba(&bytes)?;
                let img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                Some(ctx.load_texture(format!("land-{id}"), img, egui::TextureOptions::LINEAR))
            })
            .as_ref()
    }

    /// Looks again for the ones still missing (more have been drawn).
    pub fn refresh(&mut self) {
        self.loaded.retain(|_, t| t.is_some());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Rotation,
    Display,
    Live,
    Buddy,
    Content,
    Archive,
    Export,
    General,
    About,
}

/// The page's margins in Settings, left and right of it.
const PAGE_LEFT: f32 = space::S6 + 4.0;
const PAGE_RIGHT: f32 = space::S6;
/// Room for the page's scroll bar beside it.
const SCROLL_GUTTER: f32 = 24.0;
/// Settings' narrowest: the sidebar, the page at its full width and its
/// scroll bar, so no row's controls are squeezed, cut off or under the bar.
pub const SETTINGS_MIN_WIDTH: f32 =
    size::SIDEBAR + PAGE_LEFT + size::PAGE_MAX + SCROLL_GUTTER + PAGE_RIGHT;

/// Where the maker is on GitHub.
pub const MAKER: &str = "yam333";
pub const MAKER_URL: &str = "https://github.com/yam333";

impl Section {
    pub const ALL: [Section; 9] = [
        Section::Rotation,
        Section::Display,
        Section::Live,
        Section::Buddy,
        Section::Content,
        Section::Archive,
        Section::Export,
        Section::General,
        Section::About,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::Rotation => "Rotation",
            Section::Display => "Display",
            Section::Live => "Live",
            Section::Buddy => "Clawd",
            Section::Content => "Content",
            Section::Archive => "Archive",
            Section::Export => "Export",
            Section::General => "General",
            Section::About => "About",
        }
    }
}

/// Something only the user can allow, in the system's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Permission {
    pub name: &'static str,
    pub granted: bool,
    /// Opens where it's allowed.
    pub open: UiAction,
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
    /// macOS and Windows: what the features in use need the user to allow
    /// in the system's settings, and whether they have.
    pub permissions: Vec<Permission>,
    /// Ask, once, whether to start quietly at sign-in (the first run).
    pub ask_start_at_login: bool,

    /// The monitors the wallpaper is on: id and name ("Left (2560x1440,
    /// main)").
    pub monitors: Vec<(String, String)>,
    /// The same monitors as the system arranges them.
    pub screens: Vec<clairlune_platform::Monitor>,
    /// The wallpaper on screen is one scene spread across them.
    pub span_shown: bool,
    /// What the update check says (up to date, a newer version, an error),
    /// and the newer version's page.
    pub update_note: Option<String>,
    pub update_url: Option<String>,
    pub checking_updates: bool,
    /// Clairlune can install the newer version itself; where it can't,
    /// how to update by hand.
    pub update_in_place: bool,
    pub update_how: Option<String>,
    /// An update being downloaded or installed ("Downloading 0.16.0: 40%").
    pub installing: Option<String>,
    /// What a problem report would carry, to read before sending.
    pub report: String,
    /// What Remove Clairlune does with the app itself (`Ok`: what it
    /// removes; `Err`: how to remove it by hand).
    pub removal: Option<Result<String, String>>,
    /// A one-line result to show under the status (export done, an error).
    pub note: Option<String>,
    /// Timeline mode: the slot boundary to come ("Next slot at 19:30").
    pub timeline_next: Option<String>,
    /// Where saved images go when no folder is chosen.
    pub export_dir: String,
    /// Problems in timeline.toml (missing items, unknown ids).
    pub timeline_problems: Vec<String>,
    /// What the live wallpaper does, or why it can't run.
    pub live: Option<String>,
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
    /// The wallpaper a typed-in seed makes.
    FromSeed(u64),
    /// Changes start again (they were paused from the menu, or by Restore).
    Resume,
    /// Opens a web page (the maker's).
    OpenLink(&'static str),
    /// Pick where saved images go, in the system's folder picker.
    ChooseExportFolder,
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
    CheckUpdates,
    OpenUpdate,
    /// Download the newer version and install it in place.
    InstallUpdate,
    /// Open a new GitHub issue with the report filled in.
    ReportProblem,
    /// Remove Clairlune: the wallpaper back, its folders and the app gone.
    RemoveEverything,
    /// macOS: System Settings at General > Login Items.
    OpenLoginItems,
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
    /// A brief note over the page and when it appeared.
    pub toast: Option<(String, std::time::Instant)>,
    pub page: PageState,
    /// The scene behind the glass, and the grain over it.
    pub backdrop: Option<glass::Backdrop>,
    pub grain: Option<egui::TextureHandle>,
    /// What the window's own frame asks of the window (Windows).
    pub chrome: Option<Chrome>,
    pub lands: LandThumbs,
}

/// What a page remembers between frames.
#[derive(Default)]
pub struct PageState {
    /// Content: the monitor whose own choices are shown (`None`: the
    /// choices for all).
    pub content_monitor: Option<String>,
    /// General: the problem report's details are shown.
    pub report_open: bool,
    /// A brief note to show (the page asks; the view shows it).
    pub toast: Option<String>,
    /// Scroll the page to this offset once (UI test screenshots).
    pub scroll_to: Option<f32>,
    /// General: Remove Clairlune asked to confirm.
    pub remove_confirm: bool,
    /// The welcome: don't start with the computer (it does by default).
    pub welcome_no_login: bool,
    /// Rotation: the seed being typed in, and whether it isn't one.
    pub seed: String,
    pub seed_wrong: bool,
    /// Display: a monitor (or the main monitor's bar) being dragged on the
    /// arrangement.
    pub arrange_drag: Option<ArrangeDrag>,
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

pub fn show(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ViewState,
    settings: &mut Settings,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let screen = ui.max_rect();
    let side = Rect::from_min_max(screen.min, pos2(screen.min.x + size::SIDEBAR, screen.max.y));
    let pane = Rect::from_min_max(pos2(side.max.x, screen.min.y), screen.max);
    if st.grain.is_none() {
        st.grain = Some(glass::grain(ui.ctx()));
    }
    glass::paint(
        &ui.ctx().layer_painter(egui::LayerId::background()),
        t,
        screen,
        side,
        pane,
        st.backdrop.as_ref(),
        st.grain.as_ref(),
    );
    egui::Panel::left("sections")
        .exact_size(size::SIDEBAR)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: (18.0 + MAC_TOP) as i8,
            bottom: 14,
        }))
        .show(ui, |ui| sidebar(ui, theme, st, settings, status, actions));
    let top = if OWN_FRAME { 40.0 } else { 28.0 + MAC_TOP };
    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: PAGE_LEFT as i8,
            right: PAGE_RIGHT as i8,
            top: top as i8,
            bottom: 0,
        }))
        .show(ui, |ui| {
            header(ui, theme, st.section, settings, status);
            ui.add_space(space::S5);
            // The whole pane scrolls (its bar at the pane's edge), the page
            // keeping to its width on the left.
            let mut area = egui::ScrollArea::vertical().auto_shrink([false, true]);
            if let Some(y) = st.page.scroll_to.take() {
                area = area.vertical_scroll_offset(y);
            }
            area.show(ui, |ui| {
                ui.set_max_width(size::PAGE_MAX);
                let buddy = settings.buddy.show;
                page(
                    ui,
                    theme,
                    st.section,
                    &mut st.page,
                    &mut st.lands,
                    st.backdrop.as_ref().map(glass::Backdrop::image),
                    settings,
                    status,
                    actions,
                );
                ui.add_space(space::S6);
                if let Some(text) = st.page.toast.take() {
                    st.toast = Some((text, std::time::Instant::now()));
                }
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
    if OWN_FRAME {
        own_frame(ui, theme, st, screen);
    }
    toast(ui, theme, st);
    if status.ask_start_at_login {
        let before = settings.clone();
        welcome(ui, theme, settings, status, &mut st.page, actions);
        if *settings != before {
            actions.push(UiAction::SettingsChanged);
        }
    }
}

/// Windows: the frame Settings draws itself. Minimize and close at the top
/// right; the top edge moves the window; the edges resize it.
fn own_frame(ui: &mut Ui, theme: Theme, st: &mut ViewState, screen: Rect) {
    use winit::window::ResizeDirection as R;
    let t = colors(theme);
    let (bw, bh) = (46.0, 34.0);
    let close = Rect::from_min_size(pos2(screen.max.x - bw, screen.min.y), vec2(bw, bh));
    let minimize = close.translate(vec2(-bw, 0.0));
    egui::Area::new(egui::Id::unique("own-frame"))
        .order(egui::Order::Foreground)
        .fixed_pos(minimize.min)
        .show(ui.ctx(), |ui| {
            for (rect, is_close) in [(minimize, false), (close, true)] {
                let label = if is_close { "Close" } else { "Minimize" };
                let resp = ui.interact(rect, ui.scope_id().with(label), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
                let p = ui.painter();
                let ink = if resp.hovered() && is_close {
                    p.rect_filled(
                        rect,
                        egui::CornerRadius {
                            ne: 8,
                            ..Default::default()
                        },
                        Color32::from_rgb(0xC4, 0x2B, 0x1C),
                    );
                    Color32::WHITE
                } else {
                    if resp.hovered() {
                        p.rect_filled(rect, 0.0, alpha(t.glass.card, t.glass.chosen));
                    }
                    c(t.text)
                };
                let k = rect.center();
                let stroke = Stroke::new(1.0, ink);
                if is_close {
                    p.line_segment([k + vec2(-5.0, -5.0), k + vec2(5.0, 5.0)], stroke);
                    p.line_segment([k + vec2(5.0, -5.0), k + vec2(-5.0, 5.0)], stroke);
                } else {
                    p.line_segment([k + vec2(-5.0, 0.0), k + vec2(5.0, 0.0)], stroke);
                }
                if resp.clicked() {
                    st.chrome = Some(if is_close {
                        Chrome::Close
                    } else {
                        Chrome::Minimize
                    });
                }
            }
        });
    let Some(at) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
    let edge = 6.0;
    let (l, r) = (at.x - screen.min.x < edge, screen.max.x - at.x < edge);
    let (tp, b) = (at.y - screen.min.y < edge, screen.max.y - at.y < edge);
    let dir = match (l, r, tp, b) {
        (true, _, true, _) => Some(R::NorthWest),
        (_, true, true, _) => Some(R::NorthEast),
        (true, _, _, true) => Some(R::SouthWest),
        (_, true, _, true) => Some(R::SouthEast),
        (true, ..) => Some(R::West),
        (_, true, ..) => Some(R::East),
        (_, _, true, _) => Some(R::North),
        (_, _, _, true) => Some(R::South),
        _ => None,
    };
    if let Some(d) = dir {
        ui.ctx().set_cursor_icon(match d {
            R::West | R::East => egui::CursorIcon::ResizeHorizontal,
            R::North | R::South => egui::CursorIcon::ResizeVertical,
            R::NorthWest | R::SouthEast => egui::CursorIcon::ResizeNwSe,
            R::NorthEast | R::SouthWest => egui::CursorIcon::ResizeNeSw,
        });
        if pressed {
            st.chrome = Some(Chrome::Resize(d));
        }
    } else if pressed && at.y - screen.min.y < 36.0 && !minimize.contains(at) && !close.contains(at)
    {
        st.chrome = Some(Chrome::Drag);
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
    let fade = (a / 0.15).min(left / 0.5).clamp(0.0, 1.0);
    let t = colors(theme);
    egui::Area::new(egui::Id::unique("toast"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -space::S6))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(c(t.bg_raised).gamma_multiply(fade))
                .stroke(Stroke::new(
                    size::HAIRLINE,
                    c(t.hairline).gamma_multiply(fade),
                ))
                .corner_radius(radius::PANEL)
                .inner_margin(egui::Margin::symmetric(space::S4 as i8, space::S2 as i8))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).color(c(t.text_secondary).gamma_multiply(fade)));
                });
        });
    ui.ctx().request_repaint();
}

/// The page's title (the serif) and a line of what it's set to.
fn header(ui: &mut Ui, theme: Theme, section: Section, s: &Settings, status: &Status) {
    let t = colors(theme);
    ui.label(
        RichText::new(section.name())
            .font(style::display())
            .color(c(t.text)),
    );
    ui.add_space(2.0);
    ui.label(
        RichText::new(summary(section, s, status))
            .font(style::mono())
            .color(c(t.text_secondary)),
    );
}

/// What a page is set to, in a few words, under its title.
fn summary(section: Section, s: &Settings, status: &Status) -> String {
    use crate::settings::{RotationMode, WallpaperMode};
    match section {
        Section::Rotation if s.rotation.mode == RotationMode::Timeline => {
            "following the timeline".into()
        }
        Section::Rotation if s.schedule.paused => "paused".into(),
        Section::Rotation => {
            let every = INTERVALS
                .iter()
                .find(|(m, _)| *m == s.schedule.change_every_minutes)
                .map_or_else(
                    || format!("{} minutes", s.schedule.change_every_minutes),
                    |(_, l)| (*l).to_string(),
                );
            format!("a new scene every {every}")
        }
        Section::Display => {
            let n = status.monitors.len().max(1);
            let mode = match s.display.mode {
                WallpaperMode::Live => "live",
                WallpaperMode::Still => "still",
            };
            let look = match s.theme.mode {
                ThemeMode::FollowSystem => "like the system",
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
                ThemeMode::Schedule => "on a schedule",
            };
            format!(
                "{n} monitor{} \u{b7} {mode} \u{b7} {look}",
                if n == 1 { "" } else { "s" }
            )
        }
        Section::Live => format!("{} frames a second", s.live.smoothness_fps),
        Section::Buddy if s.buddy.show => "in every wallpaper".into(),
        Section::Buddy => "taking a break".into(),
        Section::Content => format!(
            "{} landscapes · {} activities · {} kinds of weather",
            LANDSCAPES.len(),
            ACTIVITIES.len(),
            WEATHER.len()
        ),
        Section::Archive if s.archive.keep_days == 0 => "kept forever".into(),
        Section::Archive if s.archive.keep_days == 1 => "kept for a day".into(),
        Section::Archive => format!("kept for {} days", s.archive.keep_days),
        Section::Export if s.export.sign => "PNGs and GIFs, signed".into(),
        Section::Export => "PNGs and GIFs".into(),
        Section::General if status.start_at_login == Some(true) => {
            "starts with the computer".into()
        }
        Section::General => "starts when you open it".into(),
        Section::About => format!(
            "Clairlune {} \u{b7} generator {}",
            env!("CARGO_PKG_VERSION"),
            clairlune_scene::GENERATOR_VERSION
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmented_rows_with_the_same_labels_change_alone() {
        // Two monitors' Live / Still frame rows: a click on one changes it,
        // and only it.
        let ctx = egui::Context::default();
        style::apply(&ctx, Theme::Dark);
        let mut values = [false, false];
        let mut rows = [Rect::NOTHING; 2];
        let options = [(false, "Live", true), (true, "Still frame", true)];
        let frame = |events: Vec<egui::Event>, values: &mut [bool; 2], rows: &mut [Rect; 2]| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 400.0))),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                for k in 0..2 {
                    let r =
                        ui.horizontal(|ui| segmented(ui, Theme::Dark, &mut values[k], &options));
                    rows[k] = r.response.rect;
                }
            })
            .drop_without_applying_deltas();
        };
        frame(Vec::new(), &mut values, &mut rows);
        let at = pos2(rows[0].right() - 12.0, rows[0].center().y);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            vec![egui::Event::PointerMoved(at), press(true)],
            &mut values,
            &mut rows,
        );
        frame(vec![press(false)], &mut values, &mut rows);
        frame(Vec::new(), &mut values, &mut rows);
        assert_eq!(values, [true, false]);
    }

    /// Renders Settings with the first run's question up for `frames`, each
    /// with `events`, and returns the actions asked for.
    fn ask_frames(events: &[egui::Event], frames: usize) -> Vec<UiAction> {
        let ctx = egui::Context::default();
        style::apply(&ctx, Theme::Light);
        let mut st = ViewState {
            section: Section::General,
            thumb: None,
            toast: None,
            page: PageState::default(),
            backdrop: None,
            grain: None,
            chrome: None,
            lands: LandThumbs::default(),
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
                settled_text(ui, "test-clock", value, "", CLOCK_W, |t| {
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
