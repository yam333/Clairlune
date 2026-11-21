//! The tray (menu-bar) icon and its menu (PLAN.md section 10): the
//! notification area on Windows, the menu bar on macOS, a StatusNotifierItem
//! over D-Bus on Linux (ksni; desktops without a tray host, such as stock
//! GNOME, fall back to the Settings window).

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
    Theme(ThemeMode),
    ToggleLive,
    TogglePause,
    SavePng,
    SaveGif,
    SaveToLibrary,
    OpenLibrary,
    OpenTimeline,
    OpenArchive,
    OpenSettings,
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
    follow: CheckMenuItem,
    light: CheckMenuItem,
    dark: CheckMenuItem,
    scheduled: CheckMenuItem,
    live: CheckMenuItem,
    pause: CheckMenuItem,
    save: MenuItem,
    save_gif: MenuItem,
    save_lib: MenuItem,
    library: MenuItem,
    timeline: MenuItem,
    archive: MenuItem,
    settings: MenuItem,
    quit: MenuItem,
}

impl Tray {
    pub fn new(mode: ThemeMode, paused: bool, live_on: bool) -> Result<Tray, String> {
        let new = MenuItem::new("New wallpaper", true, None);
        let on_monitor = Submenu::new("New wallpaper on", true);
        let previous = MenuItem::new("Previous wallpaper", true, None);
        let follow =
            CheckMenuItem::new("Follow system", true, mode == ThemeMode::FollowSystem, None);
        let light = CheckMenuItem::new("Light", true, mode == ThemeMode::Light, None);
        let dark = CheckMenuItem::new("Dark", true, mode == ThemeMode::Dark, None);
        let scheduled = CheckMenuItem::new("Schedule", true, mode == ThemeMode::Schedule, None);
        let theme = Submenu::new("Theme", true);
        theme
            .append_items(&[&follow, &light, &dark, &scheduled])
            .map_err(|e| e.to_string())?;
        let live = CheckMenuItem::new(
            "Live wallpaper",
            crate::live::supported().is_ok(),
            live_on,
            None,
        );
        let pause = CheckMenuItem::new("Pause changes", true, paused, None);
        let save = MenuItem::new("Save current as PNG", true, None);
        let save_gif = MenuItem::new("Save current as GIF", true, None);
        let save_lib = MenuItem::new("Save to library", true, None);
        let library = MenuItem::new("Library\u{2026}", true, None);
        let timeline = MenuItem::new("Timeline\u{2026}", true, None);
        let archive = MenuItem::new("Archive\u{2026}", true, None);
        let settings = MenuItem::new("Settings\u{2026}", true, None);
        let quit = MenuItem::new("Quit Clairlune", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &new,
            &previous,
            &PredefinedMenuItem::separator(),
            &theme,
            &live,
            &pause,
            &PredefinedMenuItem::separator(),
            &save,
            &save_gif,
            &save_lib,
            &library,
            &timeline,
            &archive,
            &settings,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let rgba = crate::icon::moon_rgba(32);
        let icon = Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip(clairlune_scene::APP_NAME)
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Tray {
            _icon: tray,
            menu,
            new,
            on_monitor,
            monitors: RefCell::new(Vec::new()),
            on_monitor_shown: Cell::new(false),
            previous,
            follow,
            light,
            dark,
            scheduled,
            live,
            pause,
            save,
            save_gif,
            save_lib,
            library,
            timeline,
            archive,
            settings,
            quit,
        })
    }

    pub fn action(&self, id: &MenuId) -> Option<Action> {
        let pairs = [
            (self.new.id(), Action::NewWallpaper),
            (self.previous.id(), Action::Previous),
            (self.follow.id(), Action::Theme(ThemeMode::FollowSystem)),
            (self.light.id(), Action::Theme(ThemeMode::Light)),
            (self.dark.id(), Action::Theme(ThemeMode::Dark)),
            (self.scheduled.id(), Action::Theme(ThemeMode::Schedule)),
            (self.live.id(), Action::ToggleLive),
            (self.pause.id(), Action::TogglePause),
            (self.save.id(), Action::SavePng),
            (self.save_gif.id(), Action::SaveGif),
            (self.save_lib.id(), Action::SaveToLibrary),
            (self.library.id(), Action::OpenLibrary),
            (self.timeline.id(), Action::OpenTimeline),
            (self.archive.id(), Action::OpenArchive),
            (self.settings.id(), Action::OpenSettings),
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
            let _ = self.menu.insert(&self.on_monitor, 1);
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

    pub fn show_paused(&self, paused: bool) {
        self.pause.set_checked(paused);
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
}
