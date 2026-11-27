//! The Image to ASCII studio window: a split-pane converter and wallpaper maker
//! (canvas on the left, live controls on the right, actions along the foot).

use super::style::{self, alpha, c, colors};
use super::view::{
    PALETTES, UiAction, group, hairline, highlight, primary_button, quiet_button, segmented,
    status_note, toggle,
};
use crate::tokens::{radius, size, space};
use clairlune_render::Clock;
use clairlune_render::img2ascii::{
    AsciiArt, AsciiColorMode, AsciiConfig, FitMode, INSPIRE_COUNT, RampPreset, RgbaImage,
    RowDensity, build_photo_package, convert_standalone, inspire_sample, render_rgba,
};
use clairlune_scene::Theme;
use egui::{
    Align, ComboBox, CornerRadius, Key, Layout, Pos2, Rect, RichText, Sense, Stroke, TextureHandle,
    TextureOptions, Ui, pos2, vec2,
};

const INSPECTOR_W: f32 = 352.0;

/// Ambient weather options offered for custom ASCII wallpapers.
const STUDIO_WEATHER: [(&str, &str); 14] = [
    ("none", "None (still)"),
    ("snow", "Snow"),
    ("rain", "Rain"),
    ("fireflies", "Fireflies"),
    ("petals", "Petals"),
    ("leaves", "Leaves"),
    ("dust", "Floating dust"),
    ("shooting_stars", "Shooting stars"),
    ("fog", "Fog"),
    ("sparkle", "Frost"),
    ("seeds", "Dandelion seeds"),
    ("sky_lanterns", "Sky lanterns"),
    ("spores", "Glowing spores"),
    ("wisps", "Wisps"),
];

#[derive(Default)]
pub struct AsciiState {
    pub image_name: String,
    pub source: Option<RgbaImage>,
    pub inspire_idx: usize,
    pub config: AsciiConfig,
    /// Preview polarity when `color_mode` supports both Light and Dark.
    pub preview_theme: Option<Theme>,
    pub hovering_file: bool,
    /// Toggle between fitting the canvas and inspecting 1:1 rasterized pixels.
    pub zoom_1to1: bool,
    /// Cached conversion and preview texture, rebuilt only when inputs change.
    cached_key: Option<CacheKey>,
    cached_art: Option<AsciiArt>,
    preview_tex: Option<TextureHandle>,
    live_pkg: Option<clairlune_render::ScenePackage>,
    live_inc: Option<clairlune_render::IncrementalFrame>,
    last_live_bucket: Option<u64>,
    checker_tex: Option<TextureHandle>,
    checker_dark: Option<bool>,
}

#[derive(Clone, PartialEq)]
struct CacheKey {
    image_name: String,
    dims: (u32, u32),
    sample_tag: u64,
    config: AsciiConfig,
    theme: Theme,
}

impl AsciiState {
    pub fn set_image(&mut self, name: String, image: RgbaImage) {
        self.image_name = name;
        self.source = Some(image);
        self.invalidate();
    }

    pub fn on_theme_changed(&mut self) {
        self.checker_tex = None;
        self.checker_dark = None;
    }

    fn invalidate(&mut self) {
        self.cached_key = None;
        self.cached_art = None;
        self.preview_tex = None;
        self.live_pkg = None;
        self.live_inc = None;
        self.last_live_bucket = None;
    }

    fn ensure_sample(&mut self, buddy: bool) {
        if self.source.is_none() {
            let (title, img) = inspire_sample(self.inspire_idx, buddy);
            self.image_name = title.to_string();
            self.source = Some(img);
            self.invalidate();
        }
    }
}

fn quick_tag(img: &RgbaImage) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let step = (img.pixels.len() / 64).max(1);
    for &b in img.pixels.iter().step_by(step) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn show(
    ui: &mut Ui,
    window_theme: Theme,
    st: &mut AsciiState,
    model: &mut super::Model,
    actions: &mut Vec<UiAction>,
) {
    st.ensure_sample(model.settings.buddy.show);
    let eff_theme = st.preview_theme.unwrap_or(window_theme);
    let t = colors(window_theme);

    // Keyboard shortcuts when no text field has focus: Ctrl/Cmd+V pastes an image,
    // Ctrl/Cmd+C copies ASCII text, [ / ] cycle showcase scenes, Z toggles 1:1 zoom.
    let editing_text = ui.ctx().text_edit_focused();
    if !editing_text {
        let paste_pressed = ui.input(|i| {
            (i.modifiers.command && i.key_pressed(Key::V))
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Paste(_) | egui::Event::PasteImage(_)))
        });
        if paste_pressed {
            actions.push(UiAction::PasteAsciiImage);
        }
        let step_inspire = ui.input(|i| {
            if !i.modifiers.command && i.key_pressed(Key::OpenBracket) {
                Some(INSPIRE_COUNT - 1)
            } else if !i.modifiers.command && i.key_pressed(Key::CloseBracket) {
                Some(1)
            } else {
                None
            }
        });
        if let Some(delta) = step_inspire {
            st.inspire_idx = (st.inspire_idx + delta) % INSPIRE_COUNT;
            let (title, img) = inspire_sample(st.inspire_idx, model.settings.buddy.show);
            st.set_image(title.to_string(), img);
        }
        if ui.input(|i| !i.modifiers.command && i.key_pressed(Key::Z)) {
            st.zoom_1to1 = !st.zoom_1to1;
        }
    }

    update_cache(ui.ctx(), st, eff_theme);

    if !editing_text
        && ui.input(|i| {
            (i.modifiers.command && i.key_pressed(Key::C))
                || i.events.iter().any(|e| matches!(e, egui::Event::Copy))
        })
        && let Some(art) = &st.cached_art
    {
        ui.ctx().copy_text(art.text.clone());
        actions.push(UiAction::AsciiStatusNote(format!(
            "Copied {}×{} ASCII text to the clipboard.",
            art.cols, art.rows
        )));
    }

    // Bottom action bar across the window.
    egui::Panel::bottom("ascii-actions")
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_sidebar))
                .inner_margin(egui::Margin::symmetric(space::S5 as i8, space::S3 as i8)),
        )
        .show(ui, |ui| {
            action_bar(ui, window_theme, eff_theme, st, model, actions);
        });

    // Right inspector panel with all controls.
    egui::Panel::right("ascii-inspector")
        .exact_size(INSPECTOR_W)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_sidebar))
                .inner_margin(egui::Margin::symmetric(space::S4 as i8, space::S4 as i8)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());
                inspector(ui, window_theme, eff_theme, st);
            });
        });

    // Left live preview canvas.
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(c(t.bg))
                .inner_margin(egui::Margin::symmetric(space::S5 as i8, space::S4 as i8)),
        )
        .show(ui, |ui| {
            canvas_pane(
                ui,
                window_theme,
                eff_theme,
                st,
                model.settings.buddy.show,
                actions,
            );
        });
}

fn update_cache(ctx: &egui::Context, st: &mut AsciiState, eff_theme: Theme) {
    let Some(src) = &st.source else {
        return;
    };
    let key = CacheKey {
        image_name: st.image_name.clone(),
        dims: (src.width, src.height),
        sample_tag: quick_tag(src),
        config: st.config.clone(),
        theme: eff_theme,
    };
    if st.cached_key.as_ref() != Some(&key) {
        let art = convert_standalone(src, &st.config, eff_theme);
        let preview_cell_h = (540 / art.rows.max(1)).clamp(8, 16);
        let (pw, ph, rgba) = render_rgba(&art, &st.config, eff_theme, preview_cell_h, false);
        let img = egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba);
        st.preview_tex = Some(ctx.load_texture("ascii-preview", img, TextureOptions::LINEAR));
        st.live_pkg = if st.config.has_live_weather() {
            let scale = 1.0_f64
                .min(960.0 / f64::from(pw.max(1)))
                .min(640.0 / f64::from(ph.max(1)));
            let lw = ((f64::from(pw) * scale).round() as u32).max(160);
            let lh = ((f64::from(ph) * scale).round() as u32).max(90);
            build_photo_package(src, &st.config, lw, lh, 777).ok()
        } else {
            None
        };
        st.live_inc = st.live_pkg.as_ref().and_then(|pkg| {
            let th = if pkg.themes().contains(&eff_theme) {
                eff_theme
            } else {
                pkg.themes()[0]
            };
            pkg.incremental(th)
        });
        st.last_live_bucket = None;
        st.cached_art = Some(art);
        st.cached_key = Some(key);
    }

    // Advance live weather preview when weather is active and window is focused.
    if let (Some(pkg), Some(inc)) = (&st.live_pkg, &mut st.live_inc) {
        let now_ms = (ctx.input(|i| i.time) * 1000.0) as u64;
        let bucket = now_ms / 50; // 20 fps preview
        if st.last_live_bucket != Some(bucket) {
            st.last_live_bucket = Some(bucket);
            let th = if pkg.themes().contains(&eff_theme) {
                eff_theme
            } else {
                pkg.themes()[0]
            };
            if pkg
                .advance(inc, th, false, bucket * 50, Clock::Smooth)
                .is_some()
                && let Some(pal) = pkg.palette(th)
            {
                let frame_img = inc.image();
                let rgba = clairlune_render::to_rgba(frame_img, pal);
                let cimg = egui::ColorImage::from_rgba_unmultiplied(
                    [frame_img.width as usize, frame_img.height as usize],
                    &rgba,
                );
                st.preview_tex =
                    Some(ctx.load_texture("ascii-preview", cimg, TextureOptions::LINEAR));
            }
        }
        if ctx.input(|i| i.focused) {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

fn canvas_pane(
    ui: &mut Ui,
    theme: Theme,
    eff_theme: Theme,
    st: &mut AsciiState,
    buddy: bool,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let paste_shortcut = if cfg!(target_os = "macos") {
        "Cmd+V"
    } else {
        "Ctrl+V"
    };

    ui.horizontal(|ui| {
        let buttons_reserve = 308.0;
        let left_w = (ui.available_width() - buttons_reserve).max(150.0);
        ui.allocate_ui_with_layout(vec2(left_w, 42.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(left_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Image to ASCII").font(style::title()));
                if !st.image_name.is_empty() {
                    ui.add_space(space::S2);
                    ui.add(
                        egui::Label::new(
                            RichText::new(display_title(&st.image_name)).color(c(t.text_secondary)),
                        )
                        .truncate(),
                    );
                }
            });
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "Drop an image anywhere on this window, or paste one with {paste_shortcut}."
                    ))
                    .color(c(t.text_secondary)),
                )
                .truncate(),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button("Inspire")
                .on_hover_text(format!(
                    "Next built-in showcase scene (] or [ to cycle {INSPIRE_COUNT} scenes)."
                ))
                .clicked()
            {
                st.inspire_idx = (st.inspire_idx + 1) % INSPIRE_COUNT;
                let (title, img) = inspire_sample(st.inspire_idx, buddy);
                st.set_image(title.to_string(), img);
            }
            if quiet_button(ui, theme, "\u{2039}")
                .on_hover_text("Previous showcase scene ([)")
                .clicked()
            {
                st.inspire_idx = (st.inspire_idx + INSPIRE_COUNT - 1) % INSPIRE_COUNT;
                let (title, img) = inspire_sample(st.inspire_idx, buddy);
                st.set_image(title.to_string(), img);
            }
            if ui.button("Paste").clicked() {
                actions.push(UiAction::PasteAsciiImage);
            }
            if ui.button("Choose image\u{2026}").clicked() {
                actions.push(UiAction::ChooseAsciiImage);
            }
        });
    });

    ui.add_space(space::S3);
    hairline(ui, t.hairline);
    ui.add_space(space::S3);

    let avail = ui.available_size();
    let (frame_rect, canvas_resp) = ui.allocate_exact_size(avail, Sense::click());
    if canvas_resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(if st.zoom_1to1 {
            "Click to fit preview to window (Z)"
        } else {
            "Click to inspect 1:1 pixel scale (Z)"
        })
        .clicked()
    {
        st.zoom_1to1 = !st.zoom_1to1;
    }
    let p = ui.painter();
    p.rect_filled(frame_rect, radius::PANEL, c(t.bg_raised));

    if let Some(tex) = &st.preview_tex {
        let inner = frame_rect.shrink(space::S3);
        let ts = tex.size_vec2();
        let fit_scale = (inner.width() / ts.x.max(1.0)).min(inner.height() / ts.y.max(1.0));
        let (dst, uv) = if st.zoom_1to1 {
            let shown_w = ts.x.min(inner.width());
            let shown_h = ts.y.min(inner.height());
            let dst = Rect::from_center_size(inner.center(), vec2(shown_w, shown_h));
            let uw = (shown_w / ts.x.max(1.0)).clamp(0.0, 1.0);
            let uh = (shown_h / ts.y.max(1.0)).clamp(0.0, 1.0);
            let uv = Rect::from_min_max(
                pos2(0.5 - uw * 0.5, 0.5 - uh * 0.5),
                pos2(0.5 + uw * 0.5, 0.5 + uh * 0.5),
            );
            (dst, uv)
        } else {
            let dst = Rect::from_center_size(inner.center(), ts * fit_scale);
            (dst, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)))
        };

        if st.config.color_mode == AsciiColorMode::None && st.live_pkg.is_none() {
            let want_dark = st.config.dark_canvas(eff_theme);
            if st.checker_dark != Some(want_dark) || st.checker_tex.is_none() {
                st.checker_tex = Some(make_checker(ui.ctx(), want_dark));
                st.checker_dark = Some(want_dark);
            }
            if let Some(checker) = &st.checker_tex {
                let cuv =
                    Rect::from_min_max(Pos2::ZERO, pos2(dst.width() / 24.0, dst.height() / 24.0));
                egui::Image::new(checker)
                    .uv(cuv)
                    .corner_radius(CornerRadius::same(radius::CONTROL as u8))
                    .paint_at(ui, dst);
            }
        }

        egui::Image::new(tex)
            .uv(uv)
            .corner_radius(CornerRadius::same(radius::CONTROL as u8))
            .paint_at(ui, dst);
    }

    // Quiet viewfinder readout in the lower-right corner of the canvas frame.
    if let (Some(art), Some(src)) = (&st.cached_art, &st.source) {
        let zoom_tag = if st.zoom_1to1 { "  1:1" } else { "" };
        let readout = format!(
            "{}×{} chars  {}×{} px{zoom_tag}",
            art.cols, art.rows, src.width, src.height
        );
        let p = ui.painter();
        let gal = p.layout_no_wrap(readout, style::mono(), c(t.text_secondary));
        let pad = vec2(8.0, 4.0);
        let badge_size = gal.size() + pad * 2.0;
        if badge_size.x + 24.0 < frame_rect.width() {
            let badge = Rect::from_min_size(
                pos2(
                    frame_rect.max.x - badge_size.x - 10.0,
                    frame_rect.max.y - badge_size.y - 10.0,
                ),
                badge_size,
            );
            p.rect_filled(badge, 6.0, alpha(t.bg_sidebar, 0.82));
            p.galley(badge.min + pad, gal, c(t.text_secondary));
        }
    }

    let (border_col, border_w) = if st.hovering_file {
        (c(t.accent), size::FOCUS_RING)
    } else {
        (c(t.hairline), size::HAIRLINE)
    };
    ui.painter().rect_stroke(
        frame_rect,
        radius::PANEL,
        Stroke::new(border_w, border_col),
        egui::StrokeKind::Inside,
    );

    if st.hovering_file {
        let banner = Rect::from_center_size(frame_rect.center(), vec2(260.0, 44.0));
        let p = ui.painter();
        p.rect_filled(banner, radius::CONTROL, c(t.bg_sidebar));
        p.rect_stroke(
            banner,
            radius::CONTROL,
            Stroke::new(size::FOCUS_RING, c(t.accent)),
            egui::StrokeKind::Inside,
        );
        p.text(
            banner.center(),
            egui::Align2::CENTER_CENTER,
            "Drop image to convert",
            style::body_strong(),
            c(t.text),
        );
    }
}

fn inspector(ui: &mut Ui, theme: Theme, eff_theme: Theme, st: &mut AsciiState) {
    let t = colors(theme);
    let rows_now = st.cached_art.as_ref().map_or(0, |a| a.rows);

    group(ui, "Grid");
    field_row(ui, theme, "Columns", &st.config.columns.to_string());
    let mut cols_f = st.config.columns as f32;
    if studio_slider(ui, theme, &mut cols_f, 30.0..=240.0, 1.0, 110.0, "Columns") {
        st.config.columns = cols_f.round() as u32;
    }
    ui.add_space(space::S2);

    field_row(ui, theme, "Line spacing", &format!("{rows_now} rows"));
    let density_label = |d: RowDensity| match d {
        RowDensity::Auto => "Auto, matches preview",
        RowDensity::Compact => "Compact (tight lines)",
        RowDensity::Standard => "Standard (1:2 terminal)",
        RowDensity::Tall => "Tall (wide spacing)",
    };
    ComboBox::from_id_salt("ascii-row-density")
        .selected_text(density_label(st.config.row_density))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for d in [
                RowDensity::Auto,
                RowDensity::Compact,
                RowDensity::Standard,
                RowDensity::Tall,
            ] {
                ui.selectable_value(&mut st.config.row_density, d, density_label(d));
            }
        });
    ui.add_space(space::S2);

    field_row(ui, theme, "Characters", "");
    ComboBox::from_id_salt("ascii-ramp")
        .selected_text(st.config.ramp.label())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for preset in RampPreset::ALL {
                ui.selectable_value(&mut st.config.ramp, preset, preset.label());
            }
        });
    if st.config.ramp == RampPreset::Custom {
        ui.add_space(space::S1);
        let edit_w = (ui.available_width() - 16.0).max(80.0);
        ui.add(
            egui::TextEdit::singleline(&mut st.config.custom_ramp)
                .font(style::mono())
                .char_limit(128)
                .desired_width(edit_w)
                .hint_text("@%#*+=-:. "),
        );
        caption(
            ui,
            theme,
            "Heaviest to lightest; space is added automatically.",
        );
    } else {
        let preview_ramp: String = st
            .config
            .ramp
            .heavy_to_light(&st.config.custom_ramp)
            .into_iter()
            .take(28)
            .collect();
        ui.add(
            egui::Label::new(
                RichText::new(preview_ramp.trim_end())
                    .font(style::mono())
                    .color(c(t.text_secondary)),
            )
            .truncate(),
        );
    }

    group(ui, "Tone");
    field_row(ui, theme, "Contrast", &st.config.contrast.to_string());
    let mut contrast_f = st.config.contrast as f32;
    if studio_slider(
        ui,
        theme,
        &mut contrast_f,
        50.0..=250.0,
        1.0,
        120.0,
        "Contrast",
    ) {
        st.config.contrast = contrast_f.round() as i32;
    }
    ui.add_space(space::S2);

    field_row(ui, theme, "Brightness", &st.config.brightness.to_string());
    let mut bright_f = st.config.brightness as f32;
    if studio_slider(
        ui,
        theme,
        &mut bright_f,
        -100.0..=100.0,
        1.0,
        0.0,
        "Brightness",
    ) {
        st.config.brightness = bright_f.round() as i32;
    }
    ui.add_space(space::S2);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Invert polarity").font(style::body_medium()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            toggle(ui, theme, &mut st.config.invert, "Invert polarity");
        });
    });

    group(ui, "Look");
    field_row(ui, theme, "Background", "");
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ModeTab {
        Palette,
        Photo,
        Black,
        White,
        None,
    }
    let current_tab = match &st.config.color_mode {
        AsciiColorMode::Palette(_) => ModeTab::Palette,
        AsciiColorMode::Photo => ModeTab::Photo,
        AsciiColorMode::Black => ModeTab::Black,
        AsciiColorMode::White => ModeTab::White,
        AsciiColorMode::None => ModeTab::None,
    };
    let mut picked_tab = current_tab;
    segmented_full(
        ui,
        theme,
        &mut picked_tab,
        &[
            (ModeTab::Palette, "Palette"),
            (ModeTab::Photo, "Photo"),
            (ModeTab::Black, "Black"),
            (ModeTab::White, "White"),
            (ModeTab::None, "None"),
        ],
    );
    if picked_tab != current_tab {
        st.config.color_mode = match picked_tab {
            ModeTab::Palette => AsciiColorMode::Palette("classic".into()),
            ModeTab::Photo => AsciiColorMode::Photo,
            ModeTab::Black => AsciiColorMode::Black,
            ModeTab::White => AsciiColorMode::White,
            ModeTab::None => AsciiColorMode::None,
        };
    }

    match &mut st.config.color_mode {
        AsciiColorMode::Palette(pal_id) => {
            ui.add_space(space::S2);
            let cur_name = PALETTES
                .iter()
                .find(|(id, _, _)| id == pal_id)
                .map_or("Clairlune", |(_, label, _)| *label);
            ComboBox::from_id_salt("ascii-palette")
                .selected_text(cur_name)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (id, label, _) in PALETTES {
                        ui.selectable_value(pal_id, id.to_string(), label);
                    }
                });
            ui.add_space(space::S2);
            let mut th = eff_theme;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Preview").color(c(t.text_secondary)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if segmented(
                        ui,
                        theme,
                        &mut th,
                        &[(Theme::Dark, "Dark", true), (Theme::Light, "Light", true)],
                    ) {
                        st.preview_theme = Some(th);
                    }
                });
            });
        }
        AsciiColorMode::Photo => {
            ui.add_space(space::S2);
            let mut th = eff_theme;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Preview").color(c(t.text_secondary)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if segmented(
                        ui,
                        theme,
                        &mut th,
                        &[(Theme::Dark, "Dark", true), (Theme::Light, "Light", true)],
                    ) {
                        st.preview_theme = Some(th);
                    }
                });
            });
        }
        AsciiColorMode::Black | AsciiColorMode::White => {}
        AsciiColorMode::None => {
            ui.add_space(space::S2);
            let mut th = eff_theme;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Ink").color(c(t.text_secondary)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if segmented(
                        ui,
                        theme,
                        &mut th,
                        &[(Theme::Dark, "Light", true), (Theme::Light, "Dark", true)],
                    ) {
                        st.preview_theme = Some(th);
                    }
                });
            });
            caption(ui, theme, "Transparent PNG on save and copy.");
        }
    }
    ui.add_space(space::S3);

    field_row(ui, theme, "Weather", "");
    let weather_label = STUDIO_WEATHER
        .iter()
        .find(|(id, _)| *id == st.config.weather)
        .map_or("None (still)", |(_, l)| *l);
    ComboBox::from_id_salt("ascii-weather")
        .selected_text(weather_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (id, label) in STUDIO_WEATHER {
                ui.selectable_value(&mut st.config.weather, id.to_string(), label);
            }
        });
    ui.add_space(space::S3);

    field_row(ui, theme, "Screen fit", "");
    segmented_full(
        ui,
        theme,
        &mut st.config.fit,
        &[(FitMode::Contain, "Contain"), (FitMode::Cover, "Cover")],
    );
    ui.add_space(space::S2);
}

fn action_bar(
    ui: &mut Ui,
    theme: Theme,
    eff_theme: Theme,
    st: &mut AsciiState,
    model: &super::Model,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let clean_stem = file_stem_label(&st.image_name);
    let human_title = display_title(&st.image_name);

    ui.horizontal(|ui| {
        if ui.button("Copy PNG").clicked()
            && let Some(art) = &st.cached_art
        {
            let (w, h, rgba) = render_rgba(art, &st.config, eff_theme, 18, model.settings.export.sign);
            actions.push(UiAction::CopyAsciiPng {
                width: w,
                height: h,
                rgba,
            });
        }
        if ui.button("Save PNG").clicked()
            && let Some(art) = &st.cached_art
        {
            let (w, h, rgba) = render_rgba(art, &st.config, eff_theme, 18, model.settings.export.sign);
            actions.push(UiAction::ExportAsciiPng {
                name: clean_stem.clone(),
                width: w,
                height: h,
                rgba,
            });
        }
        let (sep, _) = ui.allocate_exact_size(vec2(6.0, 18.0), Sense::hover());
        ui.painter().vline(
            sep.center().x,
            sep.y_range(),
            Stroke::new(size::HAIRLINE, c(t.hairline)),
        );
        if quiet_button(ui, theme, "Copy text").clicked()
            && let Some(art) = &st.cached_art
        {
            ui.ctx().copy_text(art.text.clone());
            actions.push(UiAction::AsciiStatusNote(format!(
                "Copied {}×{} ASCII text to the clipboard.",
                art.cols, art.rows
            )));
        }
        if quiet_button(ui, theme, "Save .txt").clicked()
            && let Some(art) = &st.cached_art
        {
            actions.push(UiAction::SaveAsciiTxt {
                name: clean_stem.clone(),
                text: art.text.clone(),
            });
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled_ui(!model.status.busy && st.source.is_some(), |ui| {
                let apply_btn = primary_button(ui, theme, "Use as wallpaper").on_hover_text(
                    "Sets this wallpaper across your monitors and pauses scheduled changes until you resume them.",
                );
                if apply_btn.clicked()
                    && let Some(src) = &st.source
                {
                    actions.push(UiAction::ApplyAsciiWallpaper {
                        name: human_title.clone(),
                        image: src.clone(),
                        config: Box::new(st.config.clone()),
                    });
                }
            });
            ui.add_enabled_ui(
                !model.status.busy && !model.status.saving_library && st.source.is_some(),
                |ui| {
                    if ui
                        .button("Save to library")
                        .on_hover_text(
                            "Saves to your Library so you can tag it or schedule it in the Timeline.",
                        )
                        .clicked()
                        && let Some(src) = &st.source
                    {
                        actions.push(UiAction::SaveAsciiToLibrary {
                            name: human_title,
                            image: src.clone(),
                            config: Box::new(st.config.clone()),
                        });
                    }
                },
            );
        });
    });

    if model.status.note.is_some() {
        ui.add_space(space::S1);
        status_note(ui, theme, model.status, actions, false);
    }
}

fn field_row(ui: &mut Ui, theme: Theme, title: &str, value: &str) {
    let t = colors(theme);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(style::body_medium())
                .color(c(t.text)),
        );
        if !value.is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(value)
                        .font(style::mono())
                        .color(c(t.text_secondary)),
                );
            });
        }
    });
    ui.add_space(2.0);
}

fn caption(ui: &mut Ui, theme: Theme, text: &str) {
    let t = colors(theme);
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .size(crate::tokens::text::SMALL.size)
                .color(c(t.text_secondary)),
        )
        .wrap(),
    );
}

/// Full-width slider styled from Clairlune's tokens; double-click resets to `default`.
fn studio_slider(
    ui: &mut Ui,
    theme: Theme,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    default: f32,
    label: &str,
) -> bool {
    let t = colors(theme);
    let w = ui.available_width();
    let h = 24.0;
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
    let min = *range.start();
    let max = *range.end();
    let span = (max - min).max(f32::EPSILON);
    let thumb_r = 7.0;
    let track_x0 = rect.left() + thumb_r;
    let track_x1 = (rect.right() - thumb_r).max(track_x0 + 1.0);

    let mut changed = false;
    if resp.double_clicked() && (*value - default).abs() > 1e-4 {
        *value = default.clamp(min, max);
        changed = true;
    } else if let Some(pos) = resp.interact_pointer_pos()
        && (resp.dragged() || resp.clicked())
    {
        let u = ((pos.x - track_x0) / (track_x1 - track_x0)).clamp(0.0, 1.0);
        let raw = min + u * span;
        let snapped = if step > 0.0 {
            ((raw - min) / step).round() * step + min
        } else {
            raw
        }
        .clamp(min, max);
        if (snapped - *value).abs() > 1e-4 {
            *value = snapped;
            changed = true;
        }
    }
    if resp.has_focus() {
        let delta = ui.input(|i| {
            let mut d = 0.0;
            if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp) {
                d += step.max(1.0);
            }
            if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown) {
                d -= step.max(1.0);
            }
            d
        });
        if delta != 0.0 {
            *value = (*value + delta).clamp(min, max);
            changed = true;
        }
    }
    if changed {
        resp.mark_changed();
    }
    resp.widget_info(|| {
        let mut info = egui::WidgetInfo::new(egui::Role::Slider);
        info.label = Some(label.into());
        info.value = Some(f64::from(*value));
        info
    });

    let u = ((*value - min) / span).clamp(0.0, 1.0);
    let cx = egui::lerp(track_x0..=track_x1, u);
    let cy = rect.center().y;
    let rail_h = 4.0;
    let rail = Rect::from_min_max(
        pos2(rect.left(), cy - rail_h * 0.5),
        pos2(rect.right(), cy + rail_h * 0.5),
    );
    let filled = Rect::from_min_max(rail.min, pos2(cx, rail.max.y));

    let p = ui.painter();
    p.rect_filled(rail, 2.0, alpha(t.border_control, 0.35));
    p.rect_filled(filled, 2.0, c(t.accent_deep));
    let thumb_at = pos2(cx, cy);
    p.circle_filled(
        thumb_at + vec2(0.0, 1.0),
        thumb_r,
        egui::Color32::from_black_alpha(65),
    );
    let thumb_col = if resp.dragged() || resp.hovered() {
        c(t.accent)
    } else {
        egui::Color32::WHITE
    };
    p.circle_filled(thumb_at, thumb_r, thumb_col);
    p.circle_stroke(
        thumb_at,
        thumb_r,
        Stroke::new(1.0, alpha(t.border_control, 0.6)),
    );
    if resp.has_focus() {
        super::view::focus_ring(ui, theme, rect, 4.0);
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if (*value - default).abs() > 1e-4 {
        resp.on_hover_text("Double-click to reset");
    }
    changed
}

/// Full-width segmented control matching `view::segmented`, sized exactly to
/// `ui.available_width()` with an animated sliding selection bead.
fn segmented_full<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: Theme,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let t = colors(theme);
    let g = &t.glass;
    let full_w = ui.available_width();
    let galleys: Vec<_> = options
        .iter()
        .map(|(v, label)| {
            let ink = if *value == *v {
                t.text
            } else {
                t.text_secondary
            };
            ui.painter()
                .layout_no_wrap((*label).to_string(), style::body(), c(ink))
        })
        .collect();
    let text_sum: f32 = galleys.iter().map(|gl| gl.size().x).sum();
    let inner_avail = (full_w - 6.0).max(text_sum);
    let extra_per_seg = (inner_avail - text_sum) / options.len().max(1) as f32;

    let base = ui.next_auto_id();
    let (track, _) = ui.allocate_exact_size(vec2(full_w, size::CONTROL + 2.0), Sense::hover());
    let groove = match theme {
        Theme::Dark => egui::Color32::from_black_alpha(80),
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

    let mut seg_spans = Vec::with_capacity(options.len());
    let mut rel_x = 3.0_f32;
    let mut active_span = None;
    for ((v, _), galley) in options.iter().zip(&galleys) {
        let w = galley.size().x + extra_per_seg;
        if *value == *v {
            active_span = Some((rel_x, w));
        }
        seg_spans.push((rel_x, w));
        rel_x += w;
    }
    if let Some((target_rx, target_w)) = active_span {
        let anim_t = ui.style().animation_time;
        let rx = ui
            .ctx()
            .animate_value_with_time(base.with("seg-full-rx"), target_rx, anim_t);
        let rw = ui
            .ctx()
            .animate_value_with_time(base.with("seg-full-rw"), target_w, anim_t);
        let bead_rect = Rect::from_min_size(
            pos2(track.min.x + rx, track.min.y + 3.0),
            vec2(rw, track.height() - 6.0),
        );
        let bead = match theme {
            Theme::Dark => alpha(g.card, g.chosen * 1.2),
            Theme::Light => egui::Color32::from_white_alpha(235),
        };
        let p = ui.painter();
        p.rect_filled(bead_rect, radius::CONTROL - 2.0, bead);
        p.hline(
            bead_rect.x_range().shrink(radius::CONTROL - 2.0),
            bead_rect.min.y + 0.5,
            Stroke::new(1.0, highlight(theme)),
        );
    }

    let mut changed = false;
    for (i, (((v, label), galley), (sx, sw))) in
        options.iter().zip(galleys).zip(seg_spans).enumerate()
    {
        let seg = Rect::from_min_size(
            pos2(track.min.x + sx, track.min.y + 3.0),
            vec2(sw, track.height() - 6.0),
        );
        let on = *value == *v;
        let resp = ui.interact(seg, base.with(("seg-full", i, *label)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, *label));
        let p = ui.painter();
        if !on && resp.hovered() {
            p.rect_filled(seg, radius::CONTROL - 2.0, alpha(g.card, g.chosen * 0.6));
        }
        p.galley(seg.center() - galley.size() / 2.0, galley, c(t.text));
        if resp.has_focus() {
            super::view::focus_ring(ui, theme, seg, radius::CONTROL - 2.0);
        }
        if resp.clicked() && !on {
            *value = *v;
            changed = true;
        }
        if !on {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    changed
}

fn make_checker(ctx: &egui::Context, dark_canvas: bool) -> TextureHandle {
    let (a, b) = if dark_canvas {
        (
            egui::Color32::from_rgb(24, 26, 32),
            egui::Color32::from_rgb(42, 45, 54),
        )
    } else {
        (
            egui::Color32::from_rgb(238, 235, 228),
            egui::Color32::from_rgb(214, 210, 200),
        )
    };
    let mut px = Vec::with_capacity(2 * 2 * 4);
    for col in [a, b, b, a] {
        px.extend_from_slice(&col.to_array());
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([2, 2], &px);
    ctx.load_texture(
        "ascii-checker",
        img,
        TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Nearest,
            wrap_mode: egui::TextureWrapMode::Repeat,
            mipmap_mode: None,
        },
    )
}

pub(crate) fn display_title(raw: &str) -> String {
    let after_dot = raw.split('·').next_back().unwrap_or(raw).trim();
    if after_dot.is_empty() {
        "Custom ASCII".into()
    } else {
        after_dot.to_string()
    }
}

pub(crate) fn file_stem_label(raw: &str) -> String {
    let part = raw.split('·').next_back().unwrap_or(raw).trim();
    let mut out = String::with_capacity(part.len());
    let mut prev_sep = false;
    for ch in part.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            out.push(ch);
            prev_sep = false;
        } else if !out.is_empty() && !prev_sep {
            out.push('-');
            prev_sep = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "ascii-image".into()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use crate::timeline::Timeline;
    use crate::ui::Model;
    use crate::ui::view::Status;
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn clean_stem_extracts_readable_file_names() {
        assert_eq!(file_stem_label("Crescent & peaks"), "Crescent-peaks");
        assert_eq!(
            file_stem_label("Sample 1 · Crescent & peaks"),
            "Crescent-peaks"
        );
        assert_eq!(file_stem_label("mountain_photo"), "mountain_photo");
        assert_eq!(file_stem_label("///"), "ascii-image");
        assert_eq!(
            display_title("Sample 1 · Crescent & peaks"),
            "Crescent & peaks"
        );
        assert_eq!(display_title("Crescent & peaks"), "Crescent & peaks");
        assert_eq!(display_title("mountain_photo"), "mountain_photo");
        assert_eq!(display_title("   "), "Custom ASCII");
    }

    #[test]
    fn studio_weather_options_all_resolve_to_live_emitters() {
        for (id, _) in STUDIO_WEATHER {
            let cfg = AsciiConfig {
                weather: id.into(),
                ..AsciiConfig::default()
            };
            assert_eq!(cfg.has_live_weather(), id != "none", "weather id {id}");
        }
    }

    fn assert_no_horizontal_clip(clip_rect: Rect, shape: &egui::Shape, win_size: egui::Vec2) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    assert_no_horizontal_clip(clip_rect, s, win_size);
                }
            }
            egui::Shape::Text(txt) => {
                let rect = txt.visual_bounding_rect();
                assert!(
                    rect.min.x >= clip_rect.min.x - 1.5 && rect.max.x <= clip_rect.max.x + 1.5,
                    "text {:?} at {:?} horizontally clipped by {:?} (window {:?})",
                    txt.galley.text(),
                    rect,
                    clip_rect,
                    win_size,
                );
            }
            egui::Shape::Rect(r) => {
                assert!(
                    r.rect.min.x >= clip_rect.min.x - 1.5 && r.rect.max.x <= clip_rect.max.x + 1.5,
                    "rect {:?} horizontally clipped by {:?} (window {:?})",
                    r.rect,
                    clip_rect,
                    win_size,
                );
            }
            egui::Shape::Circle(c_shape) => {
                let rect = c_shape.visual_bounding_rect();
                assert!(
                    rect.min.x >= clip_rect.min.x - 1.5 && rect.max.x <= clip_rect.max.x + 1.5,
                    "circle {:?} horizontally clipped by {:?} (window {:?})",
                    rect,
                    clip_rect,
                    win_size,
                );
            }
            _ => {}
        }
    }

    #[test]
    fn studio_renders_in_both_themes_without_cropping_and_copies_text() {
        let ctx = egui::Context::default();
        ctx.set_fonts(style::fonts());

        let mut settings = Settings::default();
        let status = Status::default();
        let uses = BTreeMap::new();
        let mut timeline = Timeline::default();
        let mut st = AsciiState::default();

        for (win_size, theme, mode, ramp, weather, long_name) in [
            (
                vec2(1220.0, 780.0),
                Theme::Dark,
                AsciiColorMode::Palette("classic".into()),
                RampPreset::Standard,
                "none",
                false,
            ),
            (
                vec2(940.0, 580.0),
                Theme::Light,
                AsciiColorMode::Photo,
                RampPreset::Braille,
                "snow",
                true,
            ),
            (
                vec2(940.0, 580.0),
                Theme::Dark,
                AsciiColorMode::None,
                RampPreset::Custom,
                "none",
                false,
            ),
        ] {
            style::apply(&ctx, theme);
            st.config.color_mode = mode;
            st.config.ramp = ramp;
            st.config.weather = weather.into();
            if long_name {
                st.image_name =
                    "very_long_vacation_panorama_photograph_filename_2026_11_27_final_edit.png"
                        .into();
            }
            let mut out = egui::FullOutput::default();
            // Run three frames so panel memory and scroll areas settle completely.
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, win_size)),
                    focused: true,
                    time: Some(0.25),
                    ..Default::default()
                };
                let mut actions = Vec::new();
                out = ctx.run_ui(input, |ui| {
                    let mut model = Model {
                        settings: &mut settings,
                        status: &status,
                        library: &[],
                        library_dir: Path::new("."),
                        uses: &uses,
                        timeline: &mut timeline,
                        archive: &[],
                        archive_dir: Path::new("."),
                        current_entry: None,
                    };
                    show(ui, theme, &mut st, &mut model, &mut actions);
                });
                out.textures_delta.clear();
            }
            assert!(st.cached_art.is_some());
            assert!(st.preview_tex.is_some());

            // Verify CentralPanel never overlaps Panel::right (catches egui Panel::right
            // inner-overflow shifting visible_outer_rect.min.x to the right).
            let expected_split = win_size.x - INSPECTOR_W;
            let t_cols = colors(theme);
            let mut central_bg: Option<Rect> = None;
            let mut inspector_bg: Option<Rect> = None;
            for clipped in &out.shapes {
                if let egui::Shape::Rect(r) = &clipped.shape
                    && r.rect.height() > win_size.y * 0.5
                {
                    if r.fill == c(t_cols.bg) && r.rect.min.x < 4.0 {
                        central_bg = Some(r.rect);
                    } else if r.fill == c(t_cols.bg_sidebar) && r.rect.max.x > win_size.x - 4.0 {
                        inspector_bg = Some(r.rect);
                    }
                }
                assert_no_horizontal_clip(clipped.clip_rect, &clipped.shape, win_size);
            }
            let central_bg = central_bg.expect("CentralPanel background rect");
            let inspector_bg = inspector_bg.expect("Inspector Panel::right background rect");
            assert!(
                (inspector_bg.min.x - expected_split).abs() <= 2.0,
                "inspector_bg {:?} drifted from expected split {} (window {:?})",
                inspector_bg,
                expected_split,
                win_size,
            );
            assert!(
                central_bg.max.x <= inspector_bg.min.x + 0.5,
                "CentralPanel {:?} overlapped Inspector {:?}!",
                central_bg,
                inspector_bg,
            );
        }

        // Simulate Cmd/Ctrl+C (egui::Event::Copy) shortcut to copy ASCII text cleanly.
        let copy_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1220.0, 780.0))),
            events: vec![egui::Event::Copy],
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut out = ctx.run_ui(copy_input, |ui| {
            let mut model = Model {
                settings: &mut settings,
                status: &status,
                library: &[],
                library_dir: Path::new("."),
                uses: &uses,
                timeline: &mut timeline,
                archive: &[],
                archive_dir: Path::new("."),
                current_entry: None,
            };
            show(ui, Theme::Dark, &mut st, &mut model, &mut actions);
        });
        out.textures_delta.clear();
        assert!(
            out.platform_output
                .commands
                .iter()
                .any(|cmd| matches!(cmd, egui::OutputCommand::CopyText(t) if !t.is_empty())),
            "Cmd/Ctrl+C should copy non-empty ASCII text"
        );

        // Simulate ] (next Inspire) and Z (1:1 zoom toggle) shortcuts.
        let prev_idx = st.inspire_idx;
        let key_press = |k: Key| egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let nav_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1220.0, 780.0))),
            events: vec![key_press(Key::CloseBracket), key_press(Key::Z)],
            ..Default::default()
        };
        let mut out = ctx.run_ui(nav_input, |ui| {
            let mut model = Model {
                settings: &mut settings,
                status: &status,
                library: &[],
                library_dir: Path::new("."),
                uses: &uses,
                timeline: &mut timeline,
                archive: &[],
                archive_dir: Path::new("."),
                current_entry: None,
            };
            show(ui, Theme::Dark, &mut st, &mut model, &mut actions);
        });
        out.textures_delta.clear();
        assert_eq!(st.inspire_idx, (prev_idx + 1) % INSPIRE_COUNT);
        assert!(st.zoom_1to1);

        drop(st);
        ctx.tex_manager().write().take_delta().clear();
    }
}
