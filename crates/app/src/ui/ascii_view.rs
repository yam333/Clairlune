//! The Image to ASCII studio window: a split-pane converter and wallpaper maker
//! (canvas on the left, live controls on the right, actions along the foot).

use super::style::{self, alpha, c, colors};
use super::view::{PALETTES, UiAction, hairline, primary_button, segmented, toggle};
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

const INSPECTOR_W: f32 = 336.0;

/// Ambient weather options offered for custom ASCII wallpapers.
const STUDIO_WEATHER: [(&str, &str); 14] = [
    ("none", "None (still)"),
    ("snow", "Snow"),
    ("rain", "Rain"),
    ("fireflies", "Fireflies"),
    ("petals", "Petals"),
    ("leaves", "Leaves"),
    ("stars", "Stars"),
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
    pub local_note: Option<String>,
    /// Cached conversion and preview texture, rebuilt only when inputs change.
    cached_key: Option<CacheKey>,
    cached_art: Option<AsciiArt>,
    preview_tex: Option<TextureHandle>,
    live_pkg: Option<clairlune_render::ScenePackage>,
    live_inc: Option<clairlune_render::IncrementalFrame>,
    last_live_bucket: Option<u64>,
    checker_tex: Option<TextureHandle>,
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
        self.local_note = None;
        self.invalidate();
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
    // Ctrl/Cmd+C copies the converted ASCII text.
    let editing_text = ui.ctx().memory(|m| m.focused().is_some());
    if !editing_text {
        let paste_pressed = ui.input(|i| {
            (i.modifiers.command && i.key_pressed(Key::V))
                || i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
        });
        if paste_pressed {
            actions.push(UiAction::PasteAsciiImage);
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
        st.local_note = Some(format!(
            "Copied {}×{} ASCII text to the clipboard.",
            art.cols, art.rows
        ));
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
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(c(t.bg_sidebar))
                .inner_margin(egui::Margin::symmetric(space::S5 as i8, space::S4 as i8)),
        )
        .show(ui, |ui| {
            let p = ui.painter();
            let edge = ui.max_rect().left();
            p.vline(
                edge,
                ui.max_rect().y_range(),
                Stroke::new(size::HAIRLINE, c(t.hairline)),
            );
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
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
            canvas_pane(ui, window_theme, st, model.settings.buddy.show, actions);
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
        st.live_pkg = if st.config.weather != "none" {
            let lw = pw.clamp(320, 960);
            let lh = ph.clamp(180, 640);
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
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Image to ASCII").font(style::title()));
                if let Some(src) = &st.source {
                    ui.add_space(space::S2);
                    ui.label(
                        RichText::new(format!("{} · {}×{}", st.image_name, src.width, src.height))
                            .font(style::mono())
                            .color(c(t.text_secondary)),
                    );
                }
            });
            ui.label(
                RichText::new(format!(
                    "Drop an image anywhere on this window, or paste one with {paste_shortcut}."
                ))
                .color(c(t.text_secondary)),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Inspire").clicked() {
                st.inspire_idx = (st.inspire_idx + 1) % INSPIRE_COUNT;
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
    let (frame_rect, _) = ui.allocate_exact_size(avail, Sense::hover());
    let p = ui.painter();
    p.rect_filled(frame_rect, radius::PANEL, c(t.bg_raised));

    if let Some(tex) = &st.preview_tex {
        let inner = frame_rect.shrink(space::S3);
        let ts = tex.size_vec2();
        let scale = (inner.width() / ts.x.max(1.0)).min(inner.height() / ts.y.max(1.0));
        let size_px = ts * scale;
        let dst = Rect::from_center_size(inner.center(), size_px);

        if st.config.color_mode == AsciiColorMode::None && st.live_pkg.is_none() {
            let checker = st
                .checker_tex
                .get_or_insert_with(|| make_checker(ui.ctx(), theme));
            let uv = Rect::from_min_max(Pos2::ZERO, pos2(dst.width() / 24.0, dst.height() / 24.0));
            egui::Image::new(&*checker)
                .uv(uv)
                .corner_radius(CornerRadius::same(radius::CONTROL as u8))
                .paint_at(ui, dst);
        }

        egui::Image::new(tex)
            .corner_radius(CornerRadius::same(radius::CONTROL as u8))
            .paint_at(ui, dst);
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

    section_heading(
        ui,
        theme,
        "Output width",
        &format!("{} chars", st.config.columns),
    );
    let mut cols_f = st.config.columns as f32;
    if ui
        .add_sized(
            vec2(ui.available_width(), 22.0),
            egui::Slider::new(&mut cols_f, 30.0..=240.0).show_value(false),
        )
        .changed()
    {
        st.config.columns = cols_f.round() as u32;
    }
    caption(
        ui,
        theme,
        "Columns across. Height follows your image's aspect ratio.",
    );
    ui.add_space(space::S4);

    section_heading(ui, theme, "Row density", &format!("{rows_now} rows"));
    let density_label = |d: RowDensity| match d {
        RowDensity::Auto => "Auto, matches preview",
        RowDensity::Compact => "Compact (tight lines)",
        RowDensity::Standard => "Standard (1:2 terminal)",
        RowDensity::Tall => "Tall (wide line spacing)",
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
    caption(
        ui,
        theme,
        "Adjusts row count to match the line spacing where you paste text.",
    );
    ui.add_space(space::S4);

    section_heading(ui, theme, "Character ramp", "");
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
        ui.add(
            egui::TextEdit::singleline(&mut st.config.custom_ramp)
                .font(style::mono())
                .desired_width(ui.available_width())
                .hint_text("@%#*+=-:. "),
        );
        caption(
            ui,
            theme,
            "Ordered heaviest to lightest; the last character is empty space.",
        );
    } else {
        let preview_ramp: String = st
            .config
            .ramp
            .heavy_to_light(&st.config.custom_ramp)
            .into_iter()
            .take(26)
            .collect();
        caption(ui, theme, &format!("Heaviest to lightest: {preview_ramp}"));
    }
    ui.add_space(space::S4);

    section_heading(ui, theme, "Contrast", &st.config.contrast.to_string());
    let mut contrast_f = st.config.contrast as f32;
    if ui
        .add_sized(
            vec2(ui.available_width(), 22.0),
            egui::Slider::new(&mut contrast_f, 50.0..=250.0).show_value(false),
        )
        .changed()
    {
        st.config.contrast = contrast_f.round() as i32;
    }
    ui.add_space(space::S3);

    section_heading(ui, theme, "Brightness", &st.config.brightness.to_string());
    let mut bright_f = st.config.brightness as f32;
    if ui
        .add_sized(
            vec2(ui.available_width(), 22.0),
            egui::Slider::new(&mut bright_f, -100.0..=100.0).show_value(false),
        )
        .changed()
    {
        st.config.brightness = bright_f.round() as i32;
    }
    ui.add_space(space::S2);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Invert polarity").font(style::body_medium()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            toggle(ui, theme, &mut st.config.invert, "Invert polarity");
        });
    });
    ui.add_space(space::S4);

    section_heading(ui, theme, "Background & colour", "");
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
    ui.horizontal_wrapped(|ui| {
        segmented(
            ui,
            theme,
            &mut picked_tab,
            &[
                (ModeTab::Palette, "Palette", true),
                (ModeTab::Photo, "Photo", true),
                (ModeTab::Black, "Black", true),
                (ModeTab::White, "White", true),
                (ModeTab::None, "None", true),
            ],
        );
    });
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
            caption(
                ui,
                theme,
                "Wallpapers save both light and dark versions so they follow your theme.",
            );
        }
        AsciiColorMode::Photo => {
            ui.add_space(space::S2);
            let mut th = eff_theme;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Canvas").color(c(t.text_secondary)));
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
            caption(
                ui,
                theme,
                "Each character takes its colour from the corresponding area of your image.",
            );
        }
        AsciiColorMode::Black => caption(
            ui,
            theme,
            "On black the characters are the light, so bright parts fill with ink.",
        ),
        AsciiColorMode::White => caption(
            ui,
            theme,
            "On white the characters are the ink, so dark shadows fill with strokes.",
        ),
        AsciiColorMode::None => caption(
            ui,
            theme,
            "Exports and copies a PNG with a transparent background.",
        ),
    }
    ui.add_space(space::S4);

    section_heading(ui, theme, "Live weather", "");
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
    caption(
        ui,
        theme,
        "Optional ambient particles playing live over the wallpaper.",
    );
    ui.add_space(space::S4);

    section_heading(ui, theme, "Screen fit", "");
    segmented(
        ui,
        theme,
        &mut st.config.fit,
        &[
            (FitMode::Contain, "Contain", true),
            (FitMode::Cover, "Cover", true),
        ],
    );
    caption(
        ui,
        theme,
        "Contain keeps the whole picture; Cover fills the monitor edge to edge.",
    );
    ui.add_space(space::S3);
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

    ui.horizontal(|ui| {
        if ui.button("Copy text").clicked()
            && let Some(art) = &st.cached_art
        {
            ui.ctx().copy_text(art.text.clone());
            st.local_note = Some(format!(
                "Copied {}×{} ASCII text to the clipboard.",
                art.cols, art.rows
            ));
        }
        if ui.button("Save .txt").clicked()
            && let Some(art) = &st.cached_art
        {
            st.local_note = None;
            actions.push(UiAction::SaveAsciiTxt {
                name: clean_stem.clone(),
                text: art.text.clone(),
            });
        }
        if ui.button("Copy PNG").clicked()
            && let Some(art) = &st.cached_art
        {
            st.local_note = None;
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
            st.local_note = None;
            let (w, h, rgba) = render_rgba(art, &st.config, eff_theme, 18, model.settings.export.sign);
            actions.push(UiAction::ExportAsciiPng {
                name: clean_stem.clone(),
                width: w,
                height: h,
                rgba,
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
                    st.local_note = None;
                    actions.push(UiAction::ApplyAsciiWallpaper {
                        name: clean_stem.clone(),
                        image: src.clone(),
                        config: Box::new(st.config.clone()),
                    });
                }
                if ui
                    .button("Save to library")
                    .on_hover_text("Saves to your Library so you can tag it or schedule it in the Timeline.")
                    .clicked()
                    && let Some(src) = &st.source
                {
                    st.local_note = None;
                    actions.push(UiAction::SaveAsciiToLibrary {
                        name: clean_stem.clone(),
                        image: src.clone(),
                        config: Box::new(st.config.clone()),
                    });
                }
            });
        });
    });

    if let Some(note) = st.local_note.as_deref().or(model.status.note.as_deref()) {
        ui.add_space(space::S1);
        ui.label(RichText::new(note).color(c(t.text_secondary)));
    }
}

fn section_heading(ui: &mut Ui, theme: Theme, title: &str, value: &str) {
    let t = colors(theme);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title.to_uppercase())
                .font(style::label())
                .extra_letter_spacing(style::LABEL_SPACING)
                .color(c(t.text_secondary)),
        );
        if !value.is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(value).font(style::mono()).color(c(t.text)));
            });
        }
    });
    ui.add_space(space::S1);
}

fn caption(ui: &mut Ui, theme: Theme, text: &str) {
    let t = colors(theme);
    ui.label(
        RichText::new(text)
            .size(crate::tokens::text::SMALL.size)
            .color(c(t.text_secondary)),
    );
}

fn make_checker(ctx: &egui::Context, theme: Theme) -> TextureHandle {
    let t = colors(theme);
    let a = c(t.bg_raised);
    let b = alpha(t.text_disabled, 0.22);
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

pub(crate) fn file_stem_label(raw: &str) -> String {
    let before_dot = raw.split('·').next().unwrap_or(raw).trim();
    let cleaned: String = before_dot
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('-');
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
        assert_eq!(file_stem_label("Sample 1 · Crescent & peaks"), "Sample 1");
        assert_eq!(file_stem_label("mountain_photo"), "mountain_photo");
        assert_eq!(file_stem_label("///"), "ascii-image");
    }

    #[test]
    fn studio_renders_in_both_themes_and_copies_text() {
        let ctx = egui::Context::default();
        ctx.set_fonts(style::fonts());

        let mut settings = Settings::default();
        let status = Status::default();
        let uses = BTreeMap::new();
        let mut timeline = Timeline::default();
        let mut st = AsciiState::default();

        for (theme, mode, ramp, weather) in [
            (
                Theme::Dark,
                AsciiColorMode::Palette("classic".into()),
                RampPreset::Standard,
                "none",
            ),
            (
                Theme::Light,
                AsciiColorMode::Photo,
                RampPreset::Braille,
                "snow",
            ),
            (
                Theme::Dark,
                AsciiColorMode::None,
                RampPreset::Custom,
                "none",
            ),
        ] {
            style::apply(&ctx, theme);
            st.config.color_mode = mode;
            st.config.ramp = ramp;
            st.config.weather = weather.into();
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1220.0, 740.0))),
                focused: true,
                time: Some(0.25),
                ..Default::default()
            };
            let mut actions = Vec::new();
            let mut out = ctx.run_ui(input, |ui| {
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
            assert!(st.cached_art.is_some());
            assert!(st.preview_tex.is_some());
        }

        // Simulate Cmd/Ctrl+C (egui::Event::Copy) shortcut to copy ASCII text cleanly.
        let copy_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1220.0, 740.0))),
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
        drop(st);
        ctx.tex_manager().write().take_delta().clear();
    }
}
