//! The first run's welcome: what Clairlune does and the few choices that
//! matter most.

use super::*;

/// Where the app lives when no window is open: the tray, or the menu bar.
pub(crate) fn tray_name() -> &'static str {
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
pub(crate) fn welcome(
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
