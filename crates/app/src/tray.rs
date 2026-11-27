//! The tray (menu-bar) icon and its menu: the
//! notification area on Windows, the menu bar on macOS.

use crate::settings::ThemeMode;
use clairlune_platform::Monitor;
use std::cell::{Cell, RefCell};
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// What a menu click asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    NewWallpaper,
    /// A new scene on one monitor (its place in the submenu).
    NewWallpaperOn(usize),
    Previous,
    TogglePause,
    Theme(ThemeMode),
    ToggleLive,
    SavePng,
    SaveGif,
    SaveToLibrary,
    OpenSettings,
    /// Settings, on About: who made it.
    About,
    /// The newer version: install it, or open its release page.
    OpenUpdate,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    menu: Menu,
    new: MenuItem,
    /// "New wallpaper on": one item per monitor, shown with two or more.
    on_monitor: Submenu,
    monitors: RefCell<Vec<(MenuItem, String)>>,
    on_monitor_shown: Cell<bool>,
    previous: MenuItem,
    pause: MenuItem,
    follow: CheckMenuItem,
    light: CheckMenuItem,
    dark: CheckMenuItem,
    scheduled: CheckMenuItem,
    live: CheckMenuItem,
    save: MenuItem,
    save_gif: MenuItem,
    save_lib: MenuItem,
    settings: MenuItem,
    about: MenuItem,
    quit: MenuItem,
    /// "Update available": at the top of the menu while there is one.
    update: MenuItem,
    update_shown: Cell<bool>,
}

/// The colour of the mark: macOS gets black, as a template it tints itself
/// (white on a dark menu bar); elsewhere white, or black on a light taskbar,
/// where white wouldn't show.
fn mark_colour() -> [u8; 3] {
    if cfg!(target_os = "macos") || clairlune_platform::taskbar_is_light() == Some(true) {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

/// The tray mark at the size the system shows it sharp at: 36 on macOS (an
/// 18-point menu bar icon at 2x), 32 elsewhere.
fn mark() -> Result<Icon, String> {
    let size = if cfg!(target_os = "macos") { 36 } else { 32 };
    Icon::from_rgba(crate::icon::tray_rgba(size, mark_colour()), size, size)
        .map_err(|e| e.to_string())
}

fn pause_label(paused: bool) -> &'static str {
    if paused {
        "Resume changes"
    } else {
        "Pause changes"
    }
}

/// Where "New wallpaper on" sits in the menu: right after "New wallpaper",
/// shifting by one when the update notice is at the top.
fn on_monitor_index(update_shown: bool) -> usize {
    usize::from(update_shown) + 1
}

impl Tray {
    pub fn new(mode: ThemeMode, live_on: bool, paused: bool) -> Result<Tray, String> {
        // Windows draws the menu the taskbar's way, dark or light.
        clairlune_platform::set_menu_theme(clairlune_platform::taskbar_is_light().map(|l| !l));
        let new = MenuItem::new("New wallpaper", true, None);
        let on_monitor = Submenu::new("New wallpaper on", true);
        let previous = MenuItem::new("Previous wallpaper", true, None);
        let pause = MenuItem::new(pause_label(paused), true, None);
        let follow =
            CheckMenuItem::new("Follow system", true, mode == ThemeMode::FollowSystem, None);
        let light = CheckMenuItem::new("Light", true, mode == ThemeMode::Light, None);
        let dark = CheckMenuItem::new("Dark", true, mode == ThemeMode::Dark, None);
        let scheduled = CheckMenuItem::new("Schedule", true, mode == ThemeMode::Schedule, None);
        let theme = Submenu::new("Theme", true);
        theme
            .append_items(&[&follow, &light, &dark, &scheduled])
            .map_err(|e| e.to_string())?;
        let live = CheckMenuItem::new("Live wallpaper", true, live_on, None);
        // Saving: one entry, the ways in it.
        let save = MenuItem::new("As PNG", true, None);
        let save_gif = MenuItem::new("As GIF", true, None);
        let save_lib = MenuItem::new("To library", true, None);
        let save_as = Submenu::new("Save wallpaper", true);
        save_as
            .append_items(&[&save, &save_gif, &save_lib])
            .map_err(|e| e.to_string())?;
        let settings = MenuItem::new("Settings\u{2026}", true, None);
        let about = MenuItem::new("About Clairlune", true, None);
        let quit = MenuItem::new("Quit Clairlune", true, None);
        let update = MenuItem::new("Update available\u{2026}", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &new,
            &previous,
            &pause,
            &PredefinedMenuItem::separator(),
            &theme,
            &live,
            &save_as,
            &PredefinedMenuItem::separator(),
            &settings,
            &about,
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            // Windows: a double click opens the app, so a single left click
            // mustn't pop the menu up first; the menu is on the right button.
            .with_menu_on_left_click(cfg!(not(windows)))
            .with_tooltip(clairlune_scene::APP_NAME);
        // macOS tints a template icon to suit the menu bar.
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(mark()?);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(mark()?);
        let tray = builder.build().map_err(|e| e.to_string())?;
        Ok(Tray {
            _icon: tray,
            menu,
            new,
            on_monitor,
            monitors: RefCell::new(Vec::new()),
            on_monitor_shown: Cell::new(false),
            previous,
            pause,
            follow,
            light,
            dark,
            scheduled,
            live,
            save,
            save_gif,
            save_lib,
            settings,
            about,
            quit,
            update,
            update_shown: Cell::new(false),
        })
    }

    pub fn action(&self, id: &MenuId) -> Option<Action> {
        let pairs = [
            (self.new.id(), Action::NewWallpaper),
            (self.previous.id(), Action::Previous),
            (self.pause.id(), Action::TogglePause),
            (self.follow.id(), Action::Theme(ThemeMode::FollowSystem)),
            (self.light.id(), Action::Theme(ThemeMode::Light)),
            (self.dark.id(), Action::Theme(ThemeMode::Dark)),
            (self.scheduled.id(), Action::Theme(ThemeMode::Schedule)),
            (self.live.id(), Action::ToggleLive),
            (self.save.id(), Action::SavePng),
            (self.save_gif.id(), Action::SaveGif),
            (self.save_lib.id(), Action::SaveToLibrary),
            (self.settings.id(), Action::OpenSettings),
            (self.about.id(), Action::About),
            (self.update.id(), Action::OpenUpdate),
            (self.quit.id(), Action::Quit),
        ];
        if let Some(a) = pairs.into_iter().find(|(i, _)| *i == id).map(|(_, a)| a) {
            return Some(a);
        }
        self.monitors
            .borrow()
            .iter()
            .position(|(item, _)| item.id() == id)
            .map(Action::NewWallpaperOn)
    }

    /// The monitor id behind a "New wallpaper on" item.
    pub fn monitor_id(&self, i: usize) -> Option<String> {
        self.monitors.borrow().get(i).map(|(_, id)| id.clone())
    }

    /// The monitors that can each get a new scene: the submenu lists them
    /// with two or more, and is hidden otherwise.
    pub fn set_monitors(&self, monitors: &[Monitor]) {
        let mut items = self.monitors.borrow_mut();
        for (item, _) in items.drain(..) {
            let _ = self.on_monitor.remove(&item);
        }
        if monitors.len() < 2 {
            if self.on_monitor_shown.replace(false) {
                let _ = self.menu.remove(&self.on_monitor);
            }
            return;
        }
        for (m, label) in monitors.iter().zip(monitor_labels(monitors)) {
            let item = MenuItem::new(label, true, None);
            if self.on_monitor.append(&item).is_ok() {
                items.push((item, m.id.clone()));
            }
        }
        if !self.on_monitor_shown.replace(true) {
            let _ = self
                .menu
                .insert(&self.on_monitor, on_monitor_index(self.update_shown.get()));
        }
    }

    /// Shows the newer version at the top of the menu ("Update to 0.16.0"
    /// where a click installs it, "Clairlune 0.16.0 is out" where it opens
    /// the release page), or takes it away (`None`).
    pub fn show_update(&self, version: Option<&str>, in_place: bool) {
        match version {
            Some(v) => {
                self.update.set_text(if in_place {
                    format!("Update to {v}\u{2026}")
                } else {
                    format!("Clairlune {v} is out\u{2026}")
                });
                if !self.update_shown.get() {
                    let _ = self.menu.insert(&self.update, 0);
                    self.update_shown.set(true);
                }
            }
            None => {
                if self.update_shown.get() {
                    let _ = self.menu.remove(&self.update);
                    self.update_shown.set(false);
                }
            }
        }
    }

    /// Whether macOS takes the menu bar for a dark one, as its icon there tells.
    #[cfg(target_os = "macos")]
    pub fn menu_bar_dark(&self) -> Option<bool> {
        clairlune_live::menu_bar::is_dark(&*self._icon.ns_status_item()?)
    }

    /// Draws the mark again for the taskbar's colour now (Windows: the
    /// taskbar turned light or dark), and the menu with it.
    pub fn refresh_mark(&self) {
        clairlune_platform::set_menu_theme(clairlune_platform::taskbar_is_light().map(|l| !l));
        if let Ok(icon) = mark() {
            #[cfg(target_os = "macos")]
            let _ = self._icon.set_icon_templated(Some(icon));
            #[cfg(not(target_os = "macos"))]
            let _ = self._icon.set_icon(Some(icon));
        }
    }

    /// Shows the theme mode as a radio group (check items toggle themselves on
    /// click, so the state is always set from the settings).
    pub fn show_theme(&self, mode: ThemeMode) {
        self.follow.set_checked(mode == ThemeMode::FollowSystem);
        self.light.set_checked(mode == ThemeMode::Light);
        self.dark.set_checked(mode == ThemeMode::Dark);
        self.scheduled.set_checked(mode == ThemeMode::Schedule);
    }

    /// Check items toggle themselves on click; the setting decides.
    pub fn show_live(&self, on: bool) {
        self.live.set_checked(on);
    }

    /// Shows "Resume changes" while automatic changes are paused, and "Pause
    /// changes" otherwise.
    pub fn show_paused(&self, paused: bool) {
        self.pause.set_text(pause_label(paused));
    }

    pub fn set_tooltip(&self, text: &str) {
        let _ = self._icon.set_tooltip(Some(text));
    }

    /// Grey out the generating actions while a job runs.
    pub fn set_busy(&self, busy: bool) {
        self.new.set_enabled(!busy);
        self.on_monitor.set_enabled(!busy);
        self.previous.set_enabled(!busy);
    }
}

/// Names for monitors in a menu, by where they are: two side by side are
/// Left and Right (or Top and Bottom), more are numbered left to right;
/// each with its size, and the main one marked.
pub fn monitor_labels(monitors: &[Monitor]) -> Vec<String> {
    let mut order: Vec<usize> = (0..monitors.len()).collect();
    order.sort_by_key(|&i| (monitors[i].x, monitors[i].y));
    let stacked = monitors.len() == 2 && {
        let (a, b) = (&monitors[order[0]], &monitors[order[1]]);
        b.x < a.x + a.width as i32 && a.x < b.x + b.width as i32
    };
    if stacked {
        order.sort_by_key(|&i| (monitors[i].y, monitors[i].x));
    }
    let mut labels = vec![String::new(); monitors.len()];
    for (rank, &i) in order.iter().enumerate() {
        let m = &monitors[i];
        let place = match (monitors.len(), stacked, rank) {
            (2, false, 0) => "Left".to_string(),
            (2, false, _) => "Right".to_string(),
            (2, true, 0) => "Top".to_string(),
            (2, true, _) => "Bottom".to_string(),
            _ => format!("Monitor {}", rank + 1),
        };
        let main = if m.primary { ", main" } else { "" };
        labels[i] = format!("{place} ({}\u{d7}{}{main})", m.width, m.height);
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, x: i32, y: i32, w: u32, h: u32, primary: bool) -> Monitor {
        Monitor {
            id: id.into(),
            x,
            y,
            width: w,
            height: h,
            primary,
            scale: 1,
        }
    }

    #[test]
    fn monitors_are_named_by_where_they_are() {
        let side = [
            m("b", 2560, 0, 1920, 1080, false),
            m("a", 0, 0, 2560, 1440, true),
        ];
        assert_eq!(
            monitor_labels(&side),
            ["Right (1920\u{d7}1080)", "Left (2560\u{d7}1440, main)"]
        );
        // The second one on the left of the main one (negative x).
        let left = [
            m("a", 0, 0, 1920, 1080, true),
            m("b", -1920, 0, 1920, 1080, false),
        ];
        assert_eq!(monitor_labels(&left)[1], "Left (1920\u{d7}1080)");
        let stacked = [
            m("a", 0, 1080, 1920, 1080, true),
            m("b", 0, 0, 1920, 1080, false),
        ];
        assert_eq!(
            monitor_labels(&stacked),
            ["Bottom (1920\u{d7}1080, main)", "Top (1920\u{d7}1080)"]
        );
        let three = [
            m("c", 3840, 0, 1920, 1080, false),
            m("a", 0, 0, 1920, 1080, false),
            m("b", 1920, 0, 1920, 1080, true),
        ];
        assert_eq!(
            monitor_labels(&three),
            [
                "Monitor 3 (1920\u{d7}1080)",
                "Monitor 1 (1920\u{d7}1080)",
                "Monitor 2 (1920\u{d7}1080, main)"
            ]
        );
    }

    #[test]
    fn pause_and_monitor_menu_positions_stay_in_order() {
        assert_eq!(pause_label(false), "Pause changes");
        assert_eq!(pause_label(true), "Resume changes");
        assert_eq!(on_monitor_index(false), 1);
        assert_eq!(on_monitor_index(true), 2);
    }
}
