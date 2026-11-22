//! The Settings window's content (Moonglass, docs/design.md): a sidebar of
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
            Section::Buddy => "Clawd",
            Section::Content => "Content",
            Section::Archive => "Archive",
            Section::Export => "Export",
            Section::General => "General",
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
    /// The moon mark for empty states.
    pub moon: Option<egui::TextureHandle>,
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

pub(super) const LANDSCAPES: [Item; 27] = [
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
    ("meadow", "Meadow", "Rolling grass and wildflowers"),
    ("lake", "Lake", "Mountains mirrored in still water"),
    ("volcano", "Volcano", "A smoking cone and lava fields"),
    ("islands", "Islands", "Palm islets on a calm sea"),
    ("lunar", "The Moon", "Craters, with the Earth in the sky"),
    ("bamboo", "Bamboo", "A bamboo grove"),
    ("waterfall", "Waterfall", "A fall into a misty pool"),
    (
        "countryside",
        "Countryside",
        "Fields, a windmill and a barn",
    ),
    ("ruins", "Ruins", "Old columns and an arch"),
    ("savanna", "Savanna", "Golden grass and acacias"),
    ("swamp", "Swamp", "Cypress trees in still water"),
    (
        "mushrooms",
        "Giant mushrooms",
        "A grove of towering toadstools",
    ),
    ("crystals", "Crystal cave", "Glowing crystals, no sky"),
    (
        "great_tree",
        "Great tree",
        "A colossal tree hung with lanterns",
    ),
    (
        "wizard_tower",
        "Wizard's tower",
        "A crooked tower on a crag",
    ),
    (
        "standing_stones",
        "Standing stones",
        "A ring of stones with glowing runes",
    ),
    (
        "lantern_lake",
        "Lantern lake",
        "Floating lanterns, a gate in the water",
    ),
];

pub(super) const ACTIVITIES: [Item; 38] = [
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
    ("bubbles", "Bubbles", "Blowing bubbles, daytime"),
    ("meditate", "Meditating", "By a cairn"),
    ("dance", "Dancing", "Evening and night"),
    (
        "catch_snowflakes",
        "Snowflakes",
        "Catching them, when it snows",
    ),
    ("sweep", "Sweeping", "Daytime"),
    ("hot_cocoa", "Hot cocoa", "In the cold"),
    ("gardening", "Gardening", "Watering flowers, daytime"),
    ("play_flute", "Flute", "Playing the flute"),
    ("photography", "Photography", "Photos of the view"),
    ("birdwatching", "Birdwatching", "When birds are about"),
    ("sandcastle", "Sandcastle", "On sand, daytime"),
    ("yoga", "Yoga", "Stretches by a mat"),
];

/// Weather that shows as particles. Clear weather ("none", "stars") is
/// always allowed so every scene has something to fall back to.
pub(super) const WEATHER: [Item; 15] = [
    ("snow", "Snow", ""),
    ("rain", "Rain", ""),
    ("fireflies", "Fireflies", ""),
    ("leaves", "Leaves", "Falling leaves"),
    ("dust", "Dust", ""),
    ("petals", "Petals", "Blossom drifting down"),
    ("fog", "Fog", "Mist along the ground"),
    ("blizzard", "Blizzard", "Snow on a gale"),
    ("ash", "Ash", "Falling ash, by a volcano"),
    ("sparkle", "Frost", "Ice crystals glinting in the cold"),
    ("confetti", "Confetti", "Over the rooftops"),
    ("birds", "Birds", "Crossing the sky by day"),
    ("butterflies", "Butterflies", "Fluttering low, by day"),
    ("seeds", "Dandelion seeds", "Drifting on the breeze"),
    (
        "shooting_stars",
        "Shooting stars",
        "Streaking by after dark",
    ),
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
            left: space::S6 as i8 + 4,
            right: space::S6 as i8,
            top: top as i8,
            bottom: 0,
        }))
        .show(ui, |ui| {
            header(ui, theme, st.section, settings, status);
            ui.add_space(space::S5);
            let mut area = egui::ScrollArea::vertical();
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
/// The first run's welcome: what Clairlune does, the few choices that
/// matter most (live or still, theme, monitors), and whether it starts with
/// the computer. Escape or closing it answers "not at sign-in".
fn welcome(
    ui: &mut Ui,
    theme: Theme,
    s: &mut Settings,
    status: &Status,
    ps: &mut PageState,
    actions: &mut Vec<UiAction>,
) {
    use crate::settings::{ThemeMode, WallpaperMode};
    let t = colors(theme);
    let modal = egui::Modal::new(egui::Id::unique("welcome"))
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_raised))
                .stroke(Stroke::new(size::HAIRLINE, c(t.hairline)))
                .corner_radius(radius::PANEL)
                .inner_margin(egui::Margin::same(space::S5 as i8)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_max_width(460.0);
            ui.label(RichText::new("Welcome to Clairlune").font(style::title()));
            ui.add_space(space::S2);
            ui.label(
                RichText::new(
                    "A new landscape drawn in text on your desktop, with Clawd doing something in it, and a new one every hour. Everything is made on this computer.",
                )
                .color(c(t.text_secondary)),
            );
            ui.add_space(space::S4);
            let line = |ui: &mut Ui, name: &str, control: &mut dyn FnMut(&mut Ui)| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).font(style::body_strong()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| control(ui));
                });
                ui.add_space(space::S2);
            };
            line(ui, "Wallpaper", &mut |ui| {
                segmented(
                    ui,
                    theme,
                    &mut s.display.mode,
                    &[
                        (WallpaperMode::Still, "Still frame", true),
                        (WallpaperMode::Live, "Live", true),
                    ],
                );
            });
            let mut mode = match s.theme.mode {
                ThemeMode::Schedule => ThemeMode::FollowSystem,
                m => m,
            };
            let was = mode;
            line(ui, "Theme", &mut |ui| {
                segmented(
                    ui,
                    theme,
                    &mut mode,
                    &[
                        (ThemeMode::FollowSystem, "Like the system", true),
                        (ThemeMode::Light, "Light", true),
                        (ThemeMode::Dark, "Dark", true),
                    ],
                );
            });
            if mode != was {
                s.theme.mode = mode;
            }
            if status.monitors.len() > 1 {
                line(ui, "Monitors", &mut |ui| {
                    segmented(
                        ui,
                        theme,
                        &mut s.display.span,
                        &[(false, "One scene each", true), (true, "One across all", true)],
                    );
                });
            }
            let mut login = !ps.welcome_no_login;
            line(ui, "Start with the computer", &mut |ui| {
                toggle(ui, theme, &mut login, "Start with the computer");
            });
            ps.welcome_no_login = !login;
            ui.label(
                RichText::new(format!(
                    "Live plays the scene behind your icons and pauses while an app covers the screen. Clairlune keeps running in the {}; all of this, and much more, is in Settings.",
                    tray_name()
                ))
                .color(c(t.text_secondary)),
            );
            ui.add_space(space::S4);
            ui.horizontal(|ui| {
                if primary_button(ui, theme, "Start").clicked() {
                    actions.push(UiAction::AnswerStartAtLogin(login));
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

fn sidebar(
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
        let (mark, _) = ui.allocate_exact_size(vec2(20.0, 26.0), Sense::hover());
        moon(
            ui.painter(),
            Rect::from_center_size(mark.center(), vec2(19.0, 19.0)),
            c(t.text),
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
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
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
fn now_showing(
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
                moon(
                    &p,
                    Rect::from_center_size(pic.center(), vec2(30.0, 30.0)),
                    c(t.text_secondary),
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
    let live = settings.display.mode == crate::settings::WallpaperMode::Live;
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
    let title = p.layout_job(one_line(&what, style::body_medium(), c(t.text), title_w));
    p.galley(pos2(pic.min.x + 2.0, y), title, c(t.text));
    let tag_x = pic.max.x - tag_g.size().x - 2.0;
    if live {
        p.circle_filled(pos2(tag_x - 7.0, y + 9.0), 3.0, c(t.accent_text));
    }
    p.galley(pos2(tag_x, y + 3.0), tag_g, tag_ink);
    let next = match status.next_in_minutes {
        None if status.timeline_next.is_some() => status
            .timeline_next
            .clone()
            .unwrap_or_default()
            .to_lowercase(),
        Some(0) => "next one in under a minute".to_string(),
        Some(1) => "next one in 1 min".to_string(),
        Some(m) if m < 120 => format!("next one in {m} min"),
        Some(m) => format!("next one in {} h", m / 60),
        None => "changes when you ask".to_string(),
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
    if let Some(seed) = status.seed {
        let at = Rect::from_min_size(pos2(pic.min.x, y + 45.0), vec2(pic.width(), 22.0));
        seed_box(ui, theme, at, seed, &mut st.page);
    }
    if let Some(note) = &status.note {
        ui.add_space(space::S2);
        ui.label(RichText::new(note).small().color(c(t.text_secondary)));
    }
}

/// The seed in a small box: hovered it offers to copy, clicked it copies.
fn seed_box(ui: &mut Ui, theme: Theme, rect: Rect, seed: u64, ps: &mut PageState) {
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
fn squircle(rect: Rect, r: f32) -> Vec<egui::Pos2> {
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

/// One line of text, cut short with an ellipsis where it doesn't fit.
fn one_line(text: &str, font: egui::FontId, color: Color32, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, width);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job
}

/// The part of an image of `tex` size that covers a box of `target` size,
/// centred.
fn cover_uv(tex: egui::Vec2, target: egui::Vec2) -> Rect {
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
fn highlight(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_white_alpha(38),
        Theme::Light => Color32::from_white_alpha(220),
    }
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
        Section::Theme => match s.theme.mode {
            ThemeMode::FollowSystem => "like the system".into(),
            ThemeMode::Light => "always light".into(),
            ThemeMode::Dark => "always dark".into(),
            ThemeMode::Schedule => "on a schedule".into(),
        },
        Section::Display => {
            let n = status.monitors.len().max(1);
            let mode = match s.display.mode {
                WallpaperMode::Live => "live",
                WallpaperMode::Still => "still",
            };
            format!("{n} monitor{} · {mode}", if n == 1 { "" } else { "s" })
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
        Section::Archive => format!("kept for {} days", s.archive.keep_days),
        Section::Export => "PNGs and GIFs".into(),
        Section::General => format!("Clairlune {}", env!("CARGO_PKG_VERSION")),
    }
}

/// The small drawn icons.
#[derive(Clone, Copy)]
enum Icon {
    Section(Section),
    Previous,
    Keep,
}

/// Draws an icon in `r` (laid out on a 16 x 16 grid), in strokes.
fn icon(p: &Painter, icon: Icon, r: Rect, ink: Color32) {
    let k = r.width() / 16.0;
    let at = |x: f32, y: f32| pos2(r.min.x + x * k, r.min.y + y * k);
    let st = Stroke::new(1.4 * k, ink);
    let line = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            pts.iter().map(|&(x, y)| at(x, y)).collect(),
            st,
        ));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::closed_line(
            pts.iter().map(|&(x, y)| at(x, y)).collect(),
            st,
        ));
    };
    let arc = |cx: f32, cy: f32, rad: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        (0..=12)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / 12.0).to_radians();
                at(cx + rad * a.cos(), cy + rad * a.sin())
            })
            .collect()
    };
    match icon {
        Icon::Section(Section::Rotation) => {
            p.add(egui::Shape::line(arc(8.0, 8.0, 5.3, 200.0, 330.0), st));
            line(&[(10.6, 2.9), (12.6, 5.4), (9.6, 5.9)]);
            p.add(egui::Shape::line(arc(8.0, 8.0, 5.3, 20.0, 150.0), st));
            line(&[(5.4, 13.1), (3.4, 10.6), (6.4, 10.1)]);
        }
        Icon::Section(Section::Theme) => {
            p.circle_stroke(at(8.0, 8.0), 5.6 * k, st);
            let mut half = arc(8.0, 8.0, 5.6, -90.0, 90.0);
            half.push(at(8.0, 13.6));
            p.add(egui::Shape::convex_polygon(half, ink, Stroke::NONE));
        }
        Icon::Section(Section::Display) => {
            p.rect_stroke(
                Rect::from_min_max(at(1.8, 2.8), at(14.2, 11.2)),
                1.6 * k,
                st,
                egui::StrokeKind::Middle,
            );
            line(&[(5.8, 14.0), (10.2, 14.0)]);
            line(&[(8.0, 11.2), (8.0, 14.0)]);
        }
        Icon::Section(Section::Live) => {
            line(&[
                (1.5, 8.5),
                (4.3, 8.5),
                (5.9, 4.3),
                (8.9, 12.5),
                (10.6, 8.5),
                (14.5, 8.5),
            ]);
        }
        Icon::Section(Section::Buddy) => {
            p.rect_stroke(
                Rect::from_min_max(at(3.0, 3.5), at(13.0, 10.5)),
                0.8 * k,
                st,
                egui::StrokeKind::Middle,
            );
            for x in [4.6, 6.8, 9.2, 11.4] {
                line(&[(x, 10.5), (x, 13.1)]);
            }
            line(&[(1.4, 6.8), (3.0, 6.8)]);
            line(&[(13.0, 6.8), (14.6, 6.8)]);
            line(&[(6.0, 6.2), (6.0, 7.4)]);
            line(&[(10.0, 6.2), (10.0, 7.4)]);
        }
        Icon::Section(Section::Content) => {
            closed(&[
                (1.2, 13.2),
                (5.6, 6.6),
                (8.7, 10.8),
                (10.7, 8.2),
                (14.8, 13.2),
            ]);
            p.circle_stroke(at(11.6, 4.2), 1.5 * k, st);
        }
        Icon::Section(Section::Archive) => {
            p.rect_stroke(
                Rect::from_min_max(at(1.8, 2.6), at(14.2, 5.8)),
                1.0 * k,
                st,
                egui::StrokeKind::Middle,
            );
            line(&[(3.0, 5.8), (3.0, 13.2), (13.0, 13.2), (13.0, 5.8)]);
            line(&[(6.4, 8.8), (9.6, 8.8)]);
        }
        Icon::Section(Section::Export) => {
            line(&[(8.0, 1.8), (8.0, 10.2)]);
            line(&[(4.8, 5.0), (8.0, 1.8), (11.2, 5.0)]);
            line(&[(2.6, 10.2), (2.6, 13.8), (13.4, 13.8), (13.4, 10.2)]);
        }
        Icon::Section(Section::General) => {
            line(&[(2.4, 4.0), (8.6, 4.0)]);
            line(&[(12.4, 4.0), (13.6, 4.0)]);
            line(&[(2.4, 8.0), (3.8, 8.0)]);
            line(&[(7.4, 8.0), (13.6, 8.0)]);
            line(&[(2.4, 12.0), (9.6, 12.0)]);
            for (x, y) in [(10.5, 4.0), (5.6, 8.0), (11.4, 12.0)] {
                p.circle_stroke(at(x, y), 1.6 * k, st);
            }
        }
        Icon::Previous => {
            line(&[(5.5, 3.5), (2.5, 6.5), (5.5, 9.5)]);
            let mut back = vec![at(2.5, 6.5), at(9.5, 6.5)];
            back.extend(arc(9.5, 10.5, 4.0, -90.0, 90.0));
            back.push(at(7.0, 14.5));
            p.add(egui::Shape::line(back, st));
        }
        Icon::Keep => closed(&[
            (4.0, 2.5),
            (12.0, 2.5),
            (12.0, 13.5),
            (8.0, 10.8),
            (4.0, 13.5),
        ]),
    }
}

/// The crescent moon, filled.
fn moon(p: &Painter, r: Rect, ink: Color32) {
    // A disc with a second, offset disc taken out: drawn as the lit part's
    // outline, a full arc and the shadow's inner arc.
    let c0 = r.center();
    let rad = r.width() / 2.0;
    let inner = c0 + vec2(rad * 0.42, -rad * 0.38);
    let inner_r = rad * 0.86;
    let mut pts = Vec::new();
    for i in 0..=48 {
        let a = std::f32::consts::TAU * i as f32 / 48.0;
        let q = c0 + vec2(a.cos(), a.sin()) * rad;
        if (q - inner).length() > inner_r {
            pts.push(q);
        }
    }
    // Close the crescent along the shadow's edge.
    let start = pts.first().copied();
    let end = pts.last().copied();
    if let (Some(a), Some(b)) = (start, end) {
        let (aa, ab) = ((a - inner).angle(), (b - inner).angle());
        let mut sweep = aa - ab;
        if sweep < 0.0 {
            sweep += std::f32::consts::TAU;
        }
        for i in 1..24 {
            let ang = ab + sweep * i as f32 / 24.0;
            pts.push(inner + vec2(ang.cos(), ang.sin()) * inner_r);
        }
    }
    if pts.len() > 2 {
        p.add(egui::Shape::Path(egui::epaint::PathShape {
            points: pts,
            closed: true,
            fill: ink,
            stroke: egui::epaint::PathStroke::NONE,
        }));
    }
}

/// A square button with a drawn icon.
fn icon_button(ui: &mut Ui, theme: Theme, which: Icon, label: &str) -> egui::Response {
    let t = colors(theme);
    let side = size::BUTTON_PRIMARY;
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let g = &t.glass;
    let fill = if resp.hovered() {
        g.card_alpha * 3.0
    } else {
        g.card_alpha * 1.8
    };
    let p = ui.painter();
    p.rect_filled(rect, radius::CONTROL, alpha(g.card, fill));
    p.rect_stroke(
        rect,
        radius::CONTROL,
        Stroke::new(1.0, alpha(t.border_control, 0.45)),
        egui::StrokeKind::Inside,
    );
    let ink = if ui.is_enabled() {
        c(t.text)
    } else {
        c(t.text_disabled)
    };
    icon(
        p,
        which,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        ink,
    );
    if resp.has_focus() {
        focus_ring(ui, theme, rect, radius::CONTROL);
    }
    resp.on_hover_text(label)
}

pub(super) fn focus_ring(ui: &Ui, theme: Theme, rect: Rect, r: f32) {
    ui.painter().rect_stroke(
        rect.expand(size::FOCUS_OFFSET),
        r + size::FOCUS_OFFSET,
        Stroke::new(size::FOCUS_RING, c(colors(theme).accent_text)),
        egui::StrokeKind::Outside,
    );
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

pub(super) fn primary_button(ui: &mut Ui, theme: Theme, label: &str) -> egui::Response {
    primary_button_wide(ui, theme, label, 0.0)
}

/// The primary action: ember glass, at least `min_width` wide.
fn primary_button_wide(ui: &mut Ui, theme: Theme, label: &str, min_width: f32) -> egui::Response {
    let t = colors(theme);
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), style::body_strong(), c(t.on_accent));
    let w = (galley.size().x + 2.0 * space::S4).max(min_width);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, size::BUTTON_PRIMARY), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let enabled = ui.is_enabled();
    let fade = |col: Color32| {
        if enabled {
            col
        } else {
            col.gamma_multiply(0.45)
        }
    };
    let p = ui.painter();
    let fill = if resp.hovered() && enabled {
        t.accent_deep
    } else {
        t.accent
    };
    p.rect_filled(rect, radius::CONTROL, fade(c(fill)));
    // Glass: light along the top edge, the deeper ember along the bottom.
    p.hline(
        rect.x_range().shrink(radius::CONTROL),
        rect.min.y + 0.5,
        Stroke::new(1.0, fade(Color32::from_white_alpha(90))),
    );
    p.hline(
        rect.x_range().shrink(radius::CONTROL),
        rect.max.y - 0.5,
        Stroke::new(1.0, fade(c(t.accent_deep))),
    );
    let pos = rect.center() - galley.size() / 2.0;
    p.galley(pos, galley, c(t.on_accent));
    if resp.has_focus() {
        focus_ring(ui, theme, rect, radius::CONTROL);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn hairline(ui: &mut Ui, color: crate::tokens::Rgb) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c(color));
}

pub(super) fn group(ui: &mut Ui, title: &str) {
    ui.add_space(space::S3);
    let ink = ui.visuals().widgets.noninteractive.fg_stroke.color;
    ui.label(
        RichText::new(title.to_uppercase())
            .font(style::label())
            .extra_letter_spacing(style::LABEL_SPACING)
            .color(ink),
    );
    ui.add_space(space::S2);
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
        ui.label(RichText::new(name).font(style::body_medium()));
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
    // A setting that says it all in its name has no line under it.
    if !description.is_empty() {
        ui.label(RichText::new(description).color(c(t.text_secondary)));
    }
    ui.add_space(space::S4);
}

fn toggle(ui: &mut Ui, theme: Theme, on: &mut bool, label: &str) -> egui::Response {
    let t = colors(theme);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(40.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *on, label));
    let k = ui.ctx().animate_bool_responsive(resp.id, *on);
    let enabled = ui.is_enabled();
    let fade = |col: Color32| {
        if enabled {
            col
        } else {
            col.gamma_multiply(0.4)
        }
    };
    let p = ui.painter();
    let off_fill = alpha(t.border_control, 0.28);
    let track = if *on { c(t.accent_deep) } else { off_fill };
    p.rect_filled(rect, 12.0, fade(track));
    if !*on {
        p.rect_stroke(
            rect,
            12.0,
            Stroke::new(1.0, fade(alpha(t.border_control, 0.9))),
            egui::StrokeKind::Inside,
        );
    }
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, k);
    let at = pos2(x, rect.center().y);
    p.circle_filled(
        at + vec2(0.0, 1.0),
        9.0,
        fade(Color32::from_black_alpha(50)),
    );
    p.circle_filled(at, 9.0, fade(Color32::WHITE));
    if resp.has_focus() {
        focus_ring(ui, theme, rect, 12.0);
    }
    resp
}

pub(super) fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: Theme,
    value: &mut T,
    options: &[(T, &str, bool)],
) -> bool {
    let t = colors(theme);
    let g = &t.glass;
    let pad = space::S3;
    let galleys: Vec<_> = options
        .iter()
        .map(|(v, label, enabled)| {
            let ink = if *value == *v {
                t.text
            } else if *enabled {
                t.text_secondary
            } else {
                t.text_disabled
            };
            ui.painter()
                .layout_no_wrap((*label).to_string(), style::body(), c(ink))
        })
        .collect();
    let inner: f32 = galleys.iter().map(|g| g.size().x + 2.0 * pad).sum();
    let (track, _) = ui.allocate_exact_size(vec2(inner + 6.0, size::CONTROL + 2.0), Sense::hover());
    let groove = match theme {
        Theme::Dark => Color32::from_black_alpha(80),
        Theme::Light => alpha(t.text, 0.06),
    };
    ui.painter()
        .rect_filled(track, radius::CONTROL + 1.0, groove);
    ui.painter().rect_stroke(
        track,
        radius::CONTROL + 1.0,
        Stroke::new(1.0, alpha(t.border_control, 0.35)),
        egui::StrokeKind::Inside,
    );
    let mut changed = false;
    let mut x = track.min.x + 3.0;
    for (i, ((v, label, enabled), galley)) in options.iter().zip(galleys).enumerate() {
        let w = galley.size().x + 2.0 * pad;
        let seg = Rect::from_min_size(pos2(x, track.min.y + 3.0), vec2(w, track.height() - 6.0));
        x += w;
        let on = *value == *v;
        let sense = if *enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let resp = ui.interact(seg, ui.scope_id().with(("segment", i, *label)), sense);
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::RadioButton, *enabled, on, *label)
        });
        let p = ui.painter();
        if on {
            let bead = match theme {
                Theme::Dark => alpha(g.card, g.chosen * 1.2),
                Theme::Light => Color32::from_white_alpha(235),
            };
            p.rect_filled(seg, radius::CONTROL - 2.0, bead);
            p.hline(
                seg.x_range().shrink(radius::CONTROL - 2.0),
                seg.min.y + 0.5,
                Stroke::new(1.0, highlight(theme)),
            );
        } else if resp.hovered() && *enabled {
            p.rect_filled(seg, radius::CONTROL - 2.0, alpha(g.card, g.chosen * 0.6));
        }
        p.galley(seg.center() - galley.size() / 2.0, galley, c(t.text));
        if resp.has_focus() {
            focus_ring(ui, theme, seg, radius::CONTROL - 2.0);
        }
        if resp.clicked() && !on && *enabled {
            *value = *v;
            changed = true;
        }
    }
    changed
}

pub(super) fn filter_checks(ui: &mut Ui, list: &mut Vec<String>, items: &[Item], always: &[&str]) {
    filter_chips(ui, list, items, always, |_| None);
}

/// [`filter_checks`] with a glyph before each chip's name where `glyph`
/// gives one (weather: what the scene draws for it).
fn filter_chips(
    ui: &mut Ui,
    list: &mut Vec<String>,
    items: &[Item],
    always: &[&str],
    glyph: fn(&str) -> Option<&'static str>,
) {
    let all: Vec<String> = items.iter().map(|(id, _, _)| id.to_string()).collect();
    let mut on: Vec<bool> = items
        .iter()
        .map(|(id, _, _)| list.is_empty() || list.iter().any(|x| x == id))
        .collect();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
        for (i, (id, label, hint)) in items.iter().enumerate() {
            let last_one = on[i] && on.iter().filter(|x| **x).count() == 1;
            let mut r = chip(ui, on[i], glyph(id), label, !last_one);
            if !hint.is_empty() {
                r = r.on_hover_text(*hint);
            }
            if r.clicked() {
                on[i] = !on[i];
                changed = true;
            }
        }
        if on.iter().any(|o| !o) && all_link(ui) {
            on.fill(true);
            changed = true;
        }
    });
    if changed {
        *list = picked(&all, &on, always);
    }
}

/// "All", a link turning everything back on; true when clicked.
fn all_link(ui: &mut Ui) -> bool {
    let link = ui.add(
        egui::Label::new(
            RichText::new("All")
                .color(ui.visuals().hyperlink_color)
                .font(style::body()),
        )
        .sense(egui::Sense::click()),
    );
    link.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A filter list from the items' on/off states: empty when all are on.
fn picked(all: &[String], on: &[bool], always: &[&str]) -> Vec<String> {
    let chosen: Vec<String> = all
        .iter()
        .zip(on)
        .filter(|(_, o)| **o)
        .map(|(id, _)| id.clone())
        .collect();
    if chosen.len() == all.len() {
        Vec::new()
    } else {
        always.iter().map(|s| s.to_string()).chain(chosen).collect()
    }
}

/// The landscapes as tiles: a small scene of each (drawn once by the app;
/// a quiet placeholder until it's there), its name, and a tick when on. A
/// landscape that's off stays in view, dimmed. At least one stays on.
fn land_tiles(ui: &mut Ui, theme: Theme, list: &mut Vec<String>, lands: &mut LandThumbs) {
    let t = colors(theme);
    let g = &t.glass;
    let items = &LANDSCAPES;
    let all: Vec<String> = items.iter().map(|(id, _, _)| id.to_string()).collect();
    let mut on: Vec<bool> = items
        .iter()
        .map(|(id, _, _)| list.is_empty() || list.iter().any(|x| x == id))
        .collect();
    let mut changed = false;
    let gap = space::S3;
    let width = ui.available_width();
    let cols = if width >= 560.0 { 4 } else { 3 };
    let tile_w = ((width - gap * (cols - 1) as f32) / cols as f32).floor();
    let pic_h = (tile_w - 12.0) * 9.0 / 16.0;
    let tile_h = 6.0 + pic_h + 32.0;
    for row in items.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (id, label, hint) in row {
                let i = items.iter().position(|(x, _, _)| x == id).unwrap_or(0);
                let last_one = on[i] && on.iter().filter(|x| **x).count() == 1;
                let sense = if last_one {
                    Sense::hover()
                } else {
                    Sense::click()
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(tile_w, tile_h), sense);
                resp.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::CheckBox, !last_one, on[i], *label)
                });
                let lift = if resp.hovered() && !last_one {
                    2.0
                } else {
                    0.0
                };
                let r = rect.translate(vec2(0.0, -lift));
                let p = ui.painter();
                p.rect_filled(r, radius::CONTROL + 2.0, alpha(g.card, g.card_alpha * 1.3));
                p.rect_stroke(
                    r,
                    radius::CONTROL + 2.0,
                    Stroke::new(1.0, alpha(t.border_control, if on[i] { 0.35 } else { 0.2 })),
                    egui::StrokeKind::Inside,
                );
                let pic = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(tile_w - 12.0, pic_h));
                match lands.get(ui.ctx(), id) {
                    Some(tex) => {
                        let tint = if on[i] {
                            Color32::WHITE
                        } else {
                            Color32::from_rgba_unmultiplied(120, 120, 120, 150)
                        };
                        egui::Image::new(tex)
                            .uv(cover_uv(tex.size_vec2(), pic.size()))
                            .corner_radius(7)
                            .tint(tint)
                            .paint_at(ui, pic);
                    }
                    None => {
                        let p = ui.painter();
                        p.rect_filled(pic, 7.0, alpha(g.card, g.card_alpha * 2.0));
                        p.text(
                            pic.center(),
                            Align2::CENTER_CENTER,
                            "drawing…",
                            style::mono(),
                            alpha(t.text_secondary, 0.7),
                        );
                    }
                }
                let p = ui.painter();
                let ink = if on[i] {
                    c(t.text)
                } else {
                    c(t.text_secondary)
                };
                let name = p.layout_job(one_line(label, style::body(), ink, tile_w - 44.0));
                p.galley(pos2(r.min.x + 9.0, pic.max.y + 8.0), name, ink);
                let tick = pos2(r.max.x - 16.0, pic.max.y + 16.0);
                if on[i] {
                    p.circle_filled(tick, 9.0, c(t.accent_deep));
                    p.add(egui::Shape::line(
                        vec![
                            tick + vec2(-4.0, 0.2),
                            tick + vec2(-1.2, 3.0),
                            tick + vec2(4.0, -3.2),
                        ],
                        Stroke::new(1.8, c(t.on_accent)),
                    ));
                } else {
                    p.circle_stroke(tick, 8.5, Stroke::new(1.5, alpha(t.border_control, 0.8)));
                }
                if resp.has_focus() {
                    focus_ring(ui, theme, r, radius::CONTROL + 2.0);
                }
                let resp = if hint.is_empty() {
                    resp
                } else {
                    resp.on_hover_text(*hint)
                };
                if resp.clicked() {
                    on[i] = !on[i];
                    changed = true;
                }
            }
        });
        ui.add_space(gap);
    }
    if on.iter().any(|o| !o) && all_link(ui) {
        on.fill(true);
        changed = true;
    }
    if changed {
        *list = picked(&all, &on, &[]);
    }
}

/// The glyph each weather draws on the wallpaper.
fn weather_glyph(id: &str) -> Option<&'static str> {
    Some(match id {
        "snow" | "blizzard" => "*",
        "rain" => "/",
        "fireflies" => "·",
        "leaves" => ",",
        "dust" | "ash" => ".",
        "petals" => "'",
        "fog" => "~",
        "sparkle" => "+",
        "confetti" => "%",
        "birds" => "v",
        "butterflies" => "x",
        "seeds" => "*",
        "shooting_stars" => "\\",
        _ => return None,
    })
}

fn chip(ui: &mut Ui, on: bool, glyph: Option<&str>, label: &str, enabled: bool) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let t = if dark {
        &crate::tokens::DARK
    } else {
        &crate::tokens::LIGHT
    };
    let g = &t.glass;
    let text = if on { c(t.text) } else { c(t.text_secondary) };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), style::body(), text);
    let lead = match glyph {
        Some(_) => 16.0,
        None if on => 15.0,
        None => 0.0,
    };
    let pad = space::S3 + 1.0;
    let h = 30.0;
    let w = galley.size().x + 2.0 * pad + lead;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, on, label));
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let round = egui::CornerRadius::same((h / 2.0) as u8);
        let hovered = resp.hovered() && enabled;
        if on {
            let fill = if dark {
                alpha(g.card, g.chosen * if hovered { 1.4 } else { 1.0 })
            } else {
                Color32::from_white_alpha(if hovered { 255 } else { 215 })
            };
            p.rect_filled(rect, round, fill);
            p.rect_stroke(
                rect,
                round,
                Stroke::new(1.0, alpha(t.text, if dark { 0.22 } else { 0.16 })),
                egui::StrokeKind::Inside,
            );
            p.hline(
                rect.x_range().shrink(h / 2.0),
                rect.min.y + 0.5,
                Stroke::new(
                    1.0,
                    highlight(if dark { Theme::Dark } else { Theme::Light }),
                ),
            );
        } else {
            if hovered {
                p.rect_filled(rect, round, alpha(g.card, g.chosen * 0.6));
            }
            p.rect_stroke(
                rect,
                round,
                Stroke::new(1.0, alpha(t.text, 0.12)),
                egui::StrokeKind::Inside,
            );
        }
        let mut x = rect.left() + pad;
        let cy = rect.center().y;
        match glyph {
            Some(gl) => {
                let ink = if on {
                    c(t.accent_text)
                } else {
                    c(t.text_disabled)
                };
                p.text(
                    pos2(x + 4.0, cy),
                    Align2::CENTER_CENTER,
                    gl,
                    style::mono(),
                    ink,
                );
                x += lead;
            }
            None if on => {
                // A small check mark, drawn (no font has to have one).
                let pts = vec![
                    pos2(x, cy),
                    pos2(x + 3.5, cy + 3.5),
                    pos2(x + 9.0, cy - 3.5),
                ];
                p.add(egui::Shape::line(pts, Stroke::new(1.6, c(t.accent_text))));
                x += lead;
            }
            None => {}
        }
        p.galley(pos2(x, cy - galley.size().y / 2.0), galley, text);
    }
    resp.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::NotAllowed
    })
}

#[allow(clippy::too_many_arguments)]
fn page(
    ui: &mut Ui,
    theme: Theme,
    section: Section,
    ps: &mut PageState,
    lands: &mut LandThumbs,
    s: &mut Settings,
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let before = s.clone();
    match section {
        Section::Rotation => {
            use crate::settings::RotationMode;
            row(ui, theme, "Mode", "", |ui| {
                segmented(
                    ui,
                    theme,
                    &mut s.rotation.mode,
                    &[
                        (RotationMode::Generate, "Generate", true),
                        (RotationMode::Timeline, "Timeline", true),
                    ],
                );
            });
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
                ("Change wallpaper every", "")
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
                row(ui, theme, "Change at login", "", |ui| {
                    toggle(
                        ui,
                        theme,
                        &mut s.schedule.change_at_login,
                        "Change at login",
                    );
                });
            }
            if s.schedule.paused {
                row(ui, theme, "Changes are paused", "", |ui| {
                    if ui.button("Resume").clicked() {
                        actions.push(UiAction::Resume);
                    }
                });
            }
            row(
                ui,
                theme,
                "From a seed",
                "The same seed and Content choices make the same wallpaper.",
                |ui| {
                    let show = ui.button("Show").clicked();
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut ps.seed)
                            .font(style::mono())
                            .hint_text("seed")
                            .char_limit(20)
                            .desired_width(200.0),
                    );
                    let entered =
                        field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if show || entered {
                        match ps.seed.trim().parse::<u64>() {
                            Ok(seed) => {
                                ps.seed_wrong = false;
                                actions.push(UiAction::FromSeed(seed));
                            }
                            Err(_) => ps.seed_wrong = true,
                        }
                    }
                },
            );
            if ps.seed_wrong {
                ui.label(
                    RichText::new("A seed is a whole number of up to 20 digits.")
                        .color(c(t.danger)),
                );
                ui.add_space(space::S2);
            }
        }
        Section::Theme => {
            row(ui, theme, "Appearance", "", |ui| {
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
            });
            if s.theme.mode == ThemeMode::Schedule {
                theme_schedule(ui, theme, s);
            }
        }
        Section::Display => {
            use crate::settings::WallpaperMode;
            row(ui, theme, "Wallpaper mode", "", |ui| {
                segmented(
                    ui,
                    theme,
                    &mut s.display.mode,
                    &[
                        (WallpaperMode::Still, "Still frame", true),
                        (WallpaperMode::Live, "Live", true),
                    ],
                );
            });
            if s.display.mode == WallpaperMode::Live
                && let Some(live) = &status.live
            {
                ui.label(
                    RichText::new(live)
                        .font(style::mono())
                        .color(c(t.text_secondary)),
                );
                ui.add_space(space::S4);
            }
            row(ui, theme, "Monitors", "", |ui| {
                segmented(
                    ui,
                    theme,
                    &mut s.display.span,
                    &[(false, "One each", true), (true, "Spread across", true)],
                );
            });
            if status.monitors.len() > 1 && s.display.mode == WallpaperMode::Live {
                group(ui, "Each monitor");
                ui.label(
                    RichText::new(
                        "A monitor can keep the still frame while Live plays on the others. What each one shows is set under Content.",
                    )
                    .color(c(t.text_secondary)),
                );
                ui.add_space(space::S2);
                for (id, label) in &status.monitors {
                    let mut still = s.still_only(id);
                    let was = still;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).font(style::body_strong()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            segmented(
                                ui,
                                theme,
                                &mut still,
                                &[(false, "Live", true), (true, "Still frame", true)],
                            );
                        });
                    });
                    ui.add_space(space::S2);
                    if still != was {
                        s.edit_monitor(id, label, |m| m.still = still);
                    }
                }
                ui.add_space(space::S2);
            }
        }
        Section::Live => {
            // Every setting plays at the same speed; only the choppiness and
            // the work differ.
            const SMOOTHNESS: [(u32, &str); 5] = [
                (6, "Battery saver"),
                (12, "Balanced"),
                (24, "Smooth"),
                (30, "Very smooth"),
                (60, "Ultra smooth"),
            ];
            let current = SMOOTHNESS
                .iter()
                .find(|(f, _)| *f == s.live.smoothness_fps)
                .copied()
                .unwrap_or(SMOOTHNESS[1]);
            row(ui, theme, "Smoothness", "", |ui| {
                egui::ComboBox::from_id_salt("smoothness")
                    .selected_text(format!("{} \u{b7} {} fps", current.1, current.0))
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for (fps, name) in SMOOTHNESS {
                            ui.selectable_value(
                                &mut s.live.smoothness_fps,
                                fps,
                                format!("{name} \u{b7} {fps} fps"),
                            );
                        }
                    });
            });
            row(ui, theme, "Pause behind full-screen apps", "", |ui| {
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
            });
            row(ui, theme, "Battery saver on battery", "", |ui| {
                toggle(
                    ui,
                    theme,
                    &mut s.live.battery_saver_on_battery,
                    "Battery saver on battery",
                );
            });
            if cfg!(target_os = "macos") {
                row(
                    ui,
                    theme,
                    "Lighter on Retina screens",
                    "Draws the wallpaper at half the resolution on Retina screens: about a quarter of the memory and less work, with slightly softer text. Applies to the wallpaper on screen now.",
                    |ui| {
                        toggle(
                            ui,
                            theme,
                            &mut s.live.lighter_on_retina,
                            "Lighter on Retina screens",
                        );
                    },
                );
            }
            row(ui, theme, "Pause on battery", "", |ui| {
                toggle(ui, theme, &mut s.live.pause_on_battery, "Pause on battery");
            });
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
                RichText::new("New wallpapers pick from what's on here.")
                    .color(c(t.text_secondary)),
            );
            // Which monitor's choices: all of them, or one with its own.
            let span = s.display.span;
            if status.monitors.len() > 1 {
                ui.add_space(space::S3);
                if span {
                    ui.label(
                        RichText::new(
                            "The monitors share one spread-across scene, so it picks from the choices for all of them.",
                        )
                        .color(c(t.text_secondary)),
                    );
                } else {
                    let mut pick = ps
                        .content_monitor
                        .as_ref()
                        .and_then(|id| status.monitors.iter().position(|(m, _)| m == id))
                        .map_or(0, |i| i + 1);
                    let mut options = vec![(0usize, "All monitors", true)];
                    for (i, (_, label)) in status.monitors.iter().enumerate() {
                        options.push((i + 1, label.as_str(), true));
                    }
                    ui.horizontal_wrapped(|ui| {
                        segmented(ui, theme, &mut pick, &options);
                    });
                    ps.content_monitor = pick
                        .checked_sub(1)
                        .and_then(|i| status.monitors.get(i))
                        .map(|(id, _)| id.clone());
                }
                ui.add_space(space::S3);
            }
            let selected = ps
                .content_monitor
                .clone()
                .filter(|_| !span)
                .and_then(|id| status.monitors.iter().find(|(m, _)| *m == id).cloned());
            if let Some((id, label)) = &selected {
                let mut own = s.monitor(id).is_some_and(|m| m.content.is_some());
                let was = own;
                row(
                    ui,
                    theme,
                    "Its own choices",
                    "On, this monitor's new scenes pick from the choices below, set for it alone. Off, it picks from the choices for all monitors.",
                    |ui| {
                        toggle(ui, theme, &mut own, "Its own choices");
                    },
                );
                if own != was {
                    let shared = s.content.clone();
                    s.edit_monitor(id, label, |m| m.content = own.then_some(shared));
                }
            }
            // What the chips below change: the monitor's own choices, the
            // ones for all (shown, not changeable, for a monitor without
            // its own), or the ones for all.
            let own_id = selected
                .as_ref()
                .map(|(id, _)| id.clone())
                .filter(|id| s.monitor(id).is_some_and(|m| m.content.is_some()));
            let mut shown = s.content.clone();
            let editable = selected.is_none() || own_id.is_some();
            let content: &mut crate::settings::Content = match &own_id {
                Some(id) => s
                    .display
                    .monitors
                    .iter_mut()
                    .find(|m| m.id == *id)
                    .and_then(|m| m.content.as_mut())
                    .expect("checked above"),
                None if selected.is_some() => &mut shown,
                None => &mut s.content,
            };
            ui.add_enabled_ui(editable, |ui| {
                group(ui, "Landscapes");
                land_tiles(ui, theme, &mut content.archetypes, lands);
                ui.add_space(space::S4);
                group(ui, "What Clawd does");
                filter_checks(ui, &mut content.activities, &ACTIVITIES, &[]);
                ui.add_space(space::S4);
                group(ui, "Weather");
                filter_chips(
                    ui,
                    &mut content.weather,
                    &WEATHER,
                    &CLEAR_WEATHER,
                    weather_glyph,
                );
                ui.add_space(space::S4);
                group(ui, "Look");
                filter_checks(ui, &mut content.looks, &LOOKS, &[]);
                ui.add_space(space::S2);
                row(ui, theme, "Contrast", "", |ui| {
                    use crate::settings::Contrast;
                    segmented(
                        ui,
                        theme,
                        &mut content.contrast,
                        &[
                            (Contrast::Soft, "Soft", true),
                            (Contrast::Normal, "Normal", true),
                            (Contrast::High, "High", true),
                            (Contrast::Vary, "Vary", true),
                        ],
                    );
                });
                ui.add_space(space::S4);
                group(ui, "Zoom");
                filter_checks(ui, &mut content.zooms, &ZOOMS, &[]);
            });
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
            row(ui, theme, "Size limit", "", |ui| {
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
            });
            row(ui, theme, "Library", "", |ui| {
                if ui.button("Open").clicked() {
                    actions.push(UiAction::OpenLibrary);
                }
            });
            row(ui, theme, "Archive folder", "", |ui| {
                if ui.button("Open").clicked() {
                    actions.push(UiAction::OpenArchive);
                }
            });
        }
        Section::Export => {
            row(ui, theme, "Save current as PNG", "", |ui| {
                if ui.button("Save").clicked() {
                    actions.push(UiAction::SavePng);
                }
            });
            row(ui, theme, "Save current as GIF", "", |ui| {
                if ui.button("Save").clicked() {
                    actions.push(UiAction::SaveGif);
                }
            });
            ui.label(RichText::new("Export folder").font(style::body_strong()));
            let hint = "Empty: Pictures\\Clairlune";
            ui.add(
                egui::TextEdit::singleline(&mut s.export.folder)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
        }
        Section::General => {
            match status.start_at_login {
                Some(mut on) => row(ui, theme, "Start at login", "", |ui| {
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
            let icon_name = if cfg!(target_os = "macos") {
                "Show menu bar icon"
            } else {
                "Show tray icon"
            };
            row(ui, theme, icon_name, "", |ui| {
                toggle(ui, theme, &mut s.general.show_tray_icon, icon_name);
            });
            if cfg!(any(target_os = "macos", windows)) {
                group(ui, "Permissions");
                if status.permissions.is_empty() {
                    ui.label(
                        RichText::new("Nothing to allow for what's on now.")
                            .color(c(t.text_secondary)),
                    );
                    ui.add_space(space::S4);
                }
                for p in &status.permissions {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(p.name).font(style::body_strong()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if p.granted {
                                ui.label(RichText::new("Granted").color(c(t.accent)));
                            } else if ui.button("Open Settings").clicked() {
                                actions.push(p.open.clone());
                            }
                        });
                    });
                    ui.add_space(space::S3);
                }
                ui.add_space(space::S2);
            }
            row(ui, theme, "Check for updates", "Once a day", |ui| {
                toggle(ui, theme, &mut s.general.check_updates, "Check for updates");
            });
            ui.horizontal_wrapped(|ui| {
                let busy = status.checking_updates || status.installing.is_some();
                let label = if status.checking_updates {
                    "Checking\u{2026}"
                } else {
                    "Check now"
                };
                if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                    actions.push(UiAction::CheckUpdates);
                }
                if status.update_url.is_some() {
                    if status.update_in_place {
                        if status.installing.is_none()
                            && primary_button(ui, theme, "Update now").clicked()
                        {
                            actions.push(UiAction::InstallUpdate);
                        }
                        if ui.button("What's new").clicked() {
                            actions.push(UiAction::OpenUpdate);
                        }
                    } else if primary_button(ui, theme, "Download").clicked() {
                        actions.push(UiAction::OpenUpdate);
                    }
                }
                let note = status.installing.clone().unwrap_or_else(|| {
                    status
                        .update_note
                        .clone()
                        .unwrap_or_else(|| format!("Clairlune {}", env!("CARGO_PKG_VERSION")))
                });
                ui.label(RichText::new(note).color(c(t.text_secondary)));
            });
            if status.update_url.is_some()
                && status.installing.is_none()
                && let Some(how) = &status.update_how
            {
                ui.label(RichText::new(how).color(c(t.text_secondary)));
            } else if status.update_url.is_some()
                && status.update_in_place
                && status.installing.is_none()
            {
                ui.label(
                    RichText::new(
                        "Update now downloads it from GitHub, checks it, installs it and starts Clairlune again.",
                    )
                    .color(c(t.text_secondary)),
                );
            }
            ui.add_space(space::S4);
            row(ui, theme, "Quit Clairlune", "", |ui| {
                if ui.button("Quit").clicked() {
                    actions.push(UiAction::Quit);
                }
            });
            row(ui, theme, "Restore my wallpaper", "", |ui| {
                if ui.button("Restore").clicked() {
                    actions.push(UiAction::RestoreWallpaper);
                }
            });
            row(ui, theme, "Remove Clairlune", "", |ui| {
                if ui.button("Remove\u{2026}").clicked() {
                    ps.remove_confirm = !ps.remove_confirm;
                }
            });
            if ps.remove_confirm {
                egui::Frame::new()
                    .fill(c(t.bg_sidebar))
                    .stroke(Stroke::new(size::HAIRLINE, c(t.hairline)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(space::S4 as i8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Remove Clairlune from this computer?")
                                .font(style::body_strong()),
                        );
                        ui.add_space(space::S2);
                        let mut goes = vec![
                            "Your wallpaper from before Clairlune comes back.".to_string(),
                            "Starting with the computer is turned off.".to_string(),
                        ];
                        goes.push(
                            "Its settings, archive, library, saved scenes and logs are deleted. PNGs and GIFs you exported stay where you saved them."
                                .into(),
                        );
                        if let Some(Ok(app)) = &status.removal {
                            goes.push(format!("Then Clairlune quits, and {app} goes too."));
                        } else {
                            goes.push("Then Clairlune quits.".into());
                        }
                        for g in &goes {
                            ui.label(RichText::new(format!("\u{2022} {g}")).color(c(t.text_secondary)));
                        }
                        if let Some(Err(how)) = &status.removal {
                            ui.add_space(space::S1);
                            ui.label(RichText::new(how).color(c(t.text_secondary)));
                        }
                        ui.add_space(space::S3);
                        ui.horizontal(|ui| {
                            let danger = egui::Button::new(
                                RichText::new("Remove everything").color(c(t.on_accent)),
                            )
                            .fill(c(t.danger));
                            if ui.add(danger).clicked() {
                                actions.push(UiAction::RemoveEverything);
                                ps.remove_confirm = false;
                            }
                            if ui.button("Cancel").clicked() {
                                ps.remove_confirm = false;
                            }
                        });
                    });
                ui.add_space(space::S4);
            }
            row(ui, theme, "Report a problem", "", |ui| {
                if primary_button(ui, theme, "Report on GitHub").clicked() {
                    actions.push(UiAction::ReportProblem);
                }
                if ui.button("Copy details").clicked() {
                    ui.ctx().copy_text(status.report.clone());
                    ps.toast = Some("Copied the details".into());
                }
            });
            let shown = if ps.report_open {
                "Hide the details"
            } else {
                "Show the details"
            };
            if ui.link(shown).clicked() {
                ps.report_open = !ps.report_open;
            }
            if ps.report_open {
                egui::Frame::new()
                    .fill(c(t.bg_sidebar))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(space::S3 as i8))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("report")
                            .max_height(220.0)
                            .min_scrolled_height(200.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&status.report)
                                        .font(style::mono())
                                        .color(c(t.text_secondary)),
                                );
                            });
                    });
            }
            ui.add_space(space::S4);
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
