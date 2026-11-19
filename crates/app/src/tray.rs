//! The tray (menu-bar) icon and its menu (PLAN.md section 10). Windows and
//! macOS for now; Linux gets the StatusNotifier backend later.

use crate::settings::ThemeMode;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// What a menu click asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    NewWallpaper,
    Previous,
    Theme(ThemeMode),
    TogglePause,
    SavePng,
    OpenArchive,
    OpenSettings,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    new: MenuItem,
    previous: MenuItem,
    follow: CheckMenuItem,
    light: CheckMenuItem,
    dark: CheckMenuItem,
    pause: CheckMenuItem,
    save: MenuItem,
    archive: MenuItem,
    settings: MenuItem,
    quit: MenuItem,
}

impl Tray {
    pub fn new(mode: ThemeMode, paused: bool) -> Result<Tray, String> {
        let new = MenuItem::new("New wallpaper", true, None);
        let previous = MenuItem::new("Previous wallpaper", true, None);
        let follow =
            CheckMenuItem::new("Follow system", true, mode == ThemeMode::FollowSystem, None);
        let light = CheckMenuItem::new("Light", true, mode == ThemeMode::Light, None);
        let dark = CheckMenuItem::new("Dark", true, mode == ThemeMode::Dark, None);
        let theme = Submenu::new("Theme", true);
        theme
            .append_items(&[&follow, &light, &dark])
            .map_err(|e| e.to_string())?;
        let pause = CheckMenuItem::new("Pause schedule", true, paused, None);
        let save = MenuItem::new("Save current as PNG", true, None);
        let archive = MenuItem::new("Open archive folder", true, None);
        let settings = MenuItem::new("Settings (opens settings.toml)", true, None);
        let quit = MenuItem::new("Quit Clairlune", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &new,
            &previous,
            &PredefinedMenuItem::separator(),
            &theme,
            &pause,
            &PredefinedMenuItem::separator(),
            &save,
            &archive,
            &settings,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let rgba = crate::icon::moon_rgba(32);
        let icon = Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(clairlune_scene::APP_NAME)
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Tray {
            _icon: tray,
            new,
            previous,
            follow,
            light,
            dark,
            pause,
            save,
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
            (self.pause.id(), Action::TogglePause),
            (self.save.id(), Action::SavePng),
            (self.archive.id(), Action::OpenArchive),
            (self.settings.id(), Action::OpenSettings),
            (self.quit.id(), Action::Quit),
        ];
        pairs.into_iter().find(|(i, _)| *i == id).map(|(_, a)| a)
    }

    /// Shows the theme mode as a radio group (check items toggle themselves on
    /// click, so the state is always set from the settings).
    pub fn show_theme(&self, mode: ThemeMode) {
        self.follow.set_checked(mode == ThemeMode::FollowSystem);
        self.light.set_checked(mode == ThemeMode::Light);
        self.dark.set_checked(mode == ThemeMode::Dark);
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
        self.previous.set_enabled(!busy);
    }
}
