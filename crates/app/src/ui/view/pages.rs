//! Settings' pages, one arm each, and the About page's signature.

use super::*;

/// The maker's signature: the handle in the serif's italic, underlined by
/// hand in ember. It opens their GitHub.
pub(crate) fn signature(ui: &mut Ui, theme: Theme, actions: &mut Vec<UiAction>) {
    let t = colors(theme);
    let galley = ui.painter().layout_no_wrap(
        MAKER.into(),
        style::serif(crate::tokens::text::DISPLAY.size, true),
        Color32::PLACEHOLDER,
    );
    let size = galley.size();
    let (rect, resp) = ui.allocate_exact_size(size + vec2(10.0, 10.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, "yam333 on GitHub"));
    let resp = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("github.com/yam333");
    let ink = if resp.hovered() {
        c(t.accent_text)
    } else {
        c(t.text)
    };
    let hovered = resp.hovered() || resp.has_focus();
    let p = ui.painter();
    // A pen stroke under it: rising a little, then easing off.
    let (x0, x1) = (rect.min.x + 4.0, rect.min.x + size.x * 0.96);
    let y = rect.min.y + size.y + 2.0;
    let under = |u: f32| {
        let lift = (u * std::f32::consts::PI).sin() * 3.0 - u * 2.0;
        pos2(x0 + (x1 - x0) * u, y - lift)
    };
    p.galley(rect.min + vec2(2.0, 0.0), galley, ink);
    embers(ui, theme, &under, size.y, hovered);
    let stroke: Vec<egui::Pos2> = (0..=24).map(|i| under(i as f32 / 24.0)).collect();
    let width = if hovered { 2.2 } else { 1.6 };
    p.add(egui::Shape::line(
        stroke,
        Stroke::new(width, c(t.accent_deep)),
    ));
    if resp.has_focus() {
        focus_ring(ui, theme, rect, 6.0);
    }
    if resp.clicked() {
        actions.push(UiAction::OpenLink(MAKER_URL));
    }
}

/// Embers rising off the signature's pen stroke, the way they rise off the
/// wallpapers' campfires: a few, slowly, saying there's something to touch;
/// more and quicker under the pointer. `along` places a point on the stroke
/// (0 to 1); they rise about `height`.
pub(crate) fn embers(
    ui: &Ui,
    theme: Theme,
    along: &dyn Fn(f32) -> egui::Pos2,
    height: f32,
    hot: bool,
) {
    let t = colors(theme);
    let now = ui.input(|i| i.time) as f32;
    // A fixed scatter: the same embers every time, each its own pace.
    let h = |k: u32| (k.wrapping_mul(2_654_435_761) >> 8) as f32 / (1u32 << 24) as f32;
    let count = if hot { 28 } else { 16 };
    let pace = if hot { 1.8 } else { 1.0 };
    let p = ui.painter();
    for i in 0..count {
        let k = i as u32 * 7 + 1;
        let life = 2.4 + h(k) * 1.8;
        let phase = ((now * pace + h(k + 1) * life) % life) / life;
        let start = along(-0.04 + 1.08 * h(k + 2));
        let rise = height * (0.8 + 0.6 * h(k + 3));
        let sway = (now * (0.8 + h(k + 4)) + i as f32).sin() * 3.0;
        let at = start + vec2(sway * phase, -rise * phase);
        // Bright as they leave the stroke, fading as they climb.
        let glow = (phase * std::f32::consts::PI).sin().powf(0.7) * (1.0 - phase * 0.6);
        let strength = if hot { 1.0 } else { 0.8 };
        let ink = if i % 3 == 0 { t.accent } else { t.accent_deep };
        let r = 1.3 + h(k + 5) * 1.1;
        p.circle_filled(at, r * 3.2, alpha(ink, 0.16 * glow * strength));
        p.circle_filled(at, r, alpha(ink, glow * strength));
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}

/// One setting: name and control on the first line, description under it.
/// The Theme page's Schedule settings: two times of day, or sunset and
/// sunrise at a place (worked out on this computer).
pub(crate) fn theme_schedule(ui: &mut Ui, theme: Theme, s: &mut Settings) {
    let t = colors(theme);
    let note = |ui: &mut Ui, text: &str| {
        ui.label(RichText::new(text).color(c(t.text_secondary)));
        ui.add_space(space::S4);
    };
    row(ui, theme, "Follow the sun", "", |ui| {
        toggle(ui, theme, &mut s.theme.follow_sun, "Follow the sun");
    });
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
    row(ui, theme, "Dark between", "", |ui| {
        let ok = |text: &str| crate::settings::parse_clock(text).is_some();
        // Right to left: the second time first.
        let light = settled_text(
            ui,
            "theme-light-from",
            &mut s.theme.light_from,
            "",
            CLOCK_W,
            ok,
        );
        ui.label("\u{2013}");
        let dark = settled_text(
            ui,
            "theme-dark-from",
            &mut s.theme.dark_from,
            "",
            CLOCK_W,
            ok,
        );
        valid = ok(&dark) && ok(&light);
    });
    if !valid {
        note(ui, "Use 24-hour times like 19:00 and 07:00.");
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn page(
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
                row(ui, theme, "Timeline", "", |ui| {
                    if ui.button("Edit timeline").clicked() {
                        actions.push(UiAction::EditTimeline);
                    }
                });
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
                ("Days without slots change every", "")
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
            row(ui, theme, "From a seed", "", |ui| {
                let show = ui.button("Show").clicked();
                let field = ui.add(
                    egui::TextEdit::singleline(&mut ps.seed)
                        .font(style::mono())
                        .hint_text("seed")
                        .char_limit(20)
                        .desired_width(200.0),
                );
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if show || entered {
                    match ps.seed.trim().parse::<u64>() {
                        Ok(seed) => {
                            ps.seed_wrong = false;
                            ps.seed.clear();
                            actions.push(UiAction::FromSeed(seed));
                        }
                        Err(_) => ps.seed_wrong = true,
                    }
                }
            });
            if ps.seed_wrong {
                ui.label(
                    RichText::new("A seed is a whole number of up to 20 digits.")
                        .color(c(t.danger)),
                );
                ui.add_space(space::S2);
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
            row(ui, theme, "Monitors", "", |ui| {
                segmented(
                    ui,
                    theme,
                    &mut s.display.span,
                    &[(false, "One each", true), (true, "Spread across", true)],
                );
            });
            if status.monitors.len() > 1 && s.display.mode == WallpaperMode::Live {
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
            if s.display.mode != crate::settings::WallpaperMode::Live {
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
            // Which monitor's choices: all of them, or one with its own (a
            // scene spread across them picks from the choices for all).
            let span = s.display.span;
            if status.monitors.len() > 1 {
                if !span {
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
            row(ui, theme, "Keep wallpapers for", "", |ui| {
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
            });
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
            row(ui, theme, "Sign saved images", "", |ui| {
                toggle(ui, theme, &mut s.export.sign, "Sign saved images");
            });
            let custom = !s.export.folder.trim().is_empty();
            row(ui, theme, "Export folder", "", |ui| {
                if ui.button("Choose\u{2026}").clicked() {
                    actions.push(UiAction::ChooseExportFolder);
                }
                if custom && ui.button("Pictures").clicked() {
                    s.export.folder.clear();
                }
            });
            let shown = if custom {
                s.export.folder.trim().to_string()
            } else {
                status.export_dir.clone()
            };
            ui.label(
                RichText::new(shown)
                    .font(style::mono())
                    .color(c(t.text_secondary)),
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
                "Hide details"
            } else {
                "Show details"
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
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&status.report)
                                            .font(style::mono())
                                            .color(c(t.text_secondary)),
                                    )
                                    .selectable(true),
                                );
                            });
                    });
            }
            ui.add_space(space::S4);
            row(ui, theme, "Logs", "", |ui| {
                if ui.button("Open").clicked() {
                    actions.push(UiAction::OpenLogs);
                }
            });
            let detailed_text = match &status.detailed_until {
                Some(until) => format!("On until {until}, then back to normal."),
                None => String::new(),
            };
            row(ui, theme, "Detailed logging", &detailed_text, |ui| {
                let mut on = status.detailed_until.is_some();
                if toggle(ui, theme, &mut on, "Detailed logging").changed() {
                    actions.push(UiAction::SetDetailedLogging(on));
                }
            });
            row(ui, theme, "Diagnostics", "", |ui| {
                if ui.button("Export").clicked() {
                    actions.push(UiAction::ExportDiagnostics);
                }
            });
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
        }
        Section::About => {
            group(ui, "Made by");
            signature(ui, theme, actions);
            ui.add_space(space::S4);
            ui.label(
                RichText::new("If Clairlune made your desk a little nicer, say hi.")
                    .italics()
                    .color(c(t.text_secondary)),
            );
            ui.add_space(space::S5);
            hairline(ui, t.hairline);
            ui.add_space(space::S3);
            ui.label(
                RichText::new(
                    "Unofficial fan project, not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character. Code under MIT or Apache-2.0; fonts (Instrument Serif, IBM Plex Sans, JetBrains Mono) under the SIL Open Font License.",
                )
                .color(c(t.text_secondary)),
            );
        }
    }
    if *s != before {
        actions.push(UiAction::SettingsChanged);
    }
}
