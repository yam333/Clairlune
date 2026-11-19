//! UI windows on the app's single event loop (PLAN.md section 3.4). A window
//! and everything it needs (egui context, GPU device, surface) are created
//! when it opens and dropped when it closes, so nothing of the UI is held
//! while the app sits in the tray.

pub mod style;
pub mod view;

use crate::settings::Settings;
use clairlune_scene::{APP_NAME, Theme};
use egui::ViewportId;
use std::sync::Arc;
use view::{Status, UiAction, ViewState};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct SettingsWindow {
    window: Arc<Window>,
    state: egui_winit::State,
    painter: egui_wgpu::winit::Painter,
    view: ViewState,
    theme: Theme,
    /// Save the next frame to this PNG (UI review screenshots).
    pub screenshot_to: Option<std::path::PathBuf>,
}

/// What a window event means for the app.
pub enum Outcome {
    Nothing,
    Close,
}

impl SettingsWindow {
    pub fn open(event_loop: &ActiveEventLoop, theme: Theme) -> Result<SettingsWindow, String> {
        let attrs = Window::default_attributes()
            .with_title(format!("{APP_NAME} Settings"))
            .with_inner_size(winit::dpi::LogicalSize::new(880.0, 600.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(640.0, 420.0))
            .with_enabled_buttons(winit::window::WindowButtons::CLOSE)
            .with_theme(Some(match theme {
                Theme::Light => winit::window::Theme::Light,
                Theme::Dark => winit::window::Theme::Dark,
            }))
            .with_visible(false);
        let window = Arc::new(event_loop.create_window(attrs).map_err(|e| e.to_string())?);
        let ctx = egui::Context::default();
        ctx.set_fonts(style::fonts());
        style::apply(&ctx, theme);
        let mut config = egui_wgpu::WgpuConfiguration::default();
        if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup {
            // The settings window is small and mostly idle: prefer the
            // integrated GPU and leave the discrete one asleep.
            setup.power_preference = wgpu::PowerPreference::LowPower;
        }
        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            ctx.clone(),
            config,
            false,
            egui_wgpu::RendererOptions::default(),
        ));
        pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
            .map_err(|e| e.to_string())?;
        let state = egui_winit::State::new(
            ctx,
            ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            painter.max_texture_side(),
        );
        window.set_visible(true);
        window.focus_window();
        Ok(SettingsWindow {
            window,
            state,
            painter,
            view: ViewState {
                section: view::Section::Schedule,
                thumb: None,
            },
            theme,
            screenshot_to: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Restyles for a theme change while open.
    pub fn set_theme(&mut self, theme: Theme) {
        if theme != self.theme {
            self.theme = theme;
            style::apply(self.state.egui_ctx(), theme);
            self.window.request_redraw();
        }
    }

    /// Shows the current wallpaper's thumbnail (RGBA).
    pub fn set_thumb(&mut self, size: [usize; 2], rgba: &[u8]) {
        let img = egui::ColorImage::from_rgba_unmultiplied(size, rgba);
        self.view.thumb = Some(self.state.egui_ctx().load_texture(
            "current-wallpaper",
            img,
            egui::TextureOptions::LINEAR,
        ));
        self.window.request_redraw();
    }

    pub fn set_section(&mut self, section: view::Section) {
        self.view.section = section;
        self.window.request_redraw();
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Handles one window event; paints on redraw and collects the user's actions.
    pub fn on_event(
        &mut self,
        event: &WindowEvent,
        settings: &mut Settings,
        status: &Status,
        actions: &mut Vec<UiAction>,
    ) -> Outcome {
        match event {
            WindowEvent::CloseRequested => return Outcome::Close,
            WindowEvent::KeyboardInput { event: key, .. }
                if key.state.is_pressed()
                    && key.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) =>
            {
                return Outcome::Close;
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (
                    std::num::NonZeroU32::new(size.width),
                    std::num::NonZeroU32::new(size.height),
                ) {
                    self.painter.on_window_resized(ViewportId::ROOT, w, h);
                }
            }
            WindowEvent::RedrawRequested => {
                self.paint(settings, status, actions);
                return Outcome::Nothing;
            }
            _ => {}
        }
        let resp = self.state.on_window_event(&self.window, event);
        if resp.repaint {
            self.window.request_redraw();
        }
        Outcome::Nothing
    }

    fn paint(&mut self, settings: &mut Settings, status: &Status, actions: &mut Vec<UiAction>) {
        let input = self.state.take_egui_input(&self.window);
        let ctx = self.state.egui_ctx().clone();
        let theme = self.theme;
        let view = &mut self.view;
        let mut full = ctx.run_ui(input, |ui| {
            view::show(ui, theme, view, settings, status, actions);
        });
        self.state
            .handle_platform_output(&self.window, std::mem::take(&mut full.platform_output));
        let prims = ctx.tessellate(std::mem::take(&mut full.shapes), full.pixels_per_point);
        let mut capture = Vec::new();
        if let Some(path) = self.screenshot_to.take() {
            capture.push(egui::ScreenshotCallback::new(move |img| {
                if let Err(e) = save_png(&path, &img) {
                    tracing::warn!(target: "clairlune", error = %e, "screenshot");
                }
            }));
        }
        let bg = style::colors(theme).bg;
        let clear = [
            f32::from(bg.0) / 255.0,
            f32::from(bg.1) / 255.0,
            f32::from(bg.2) / 255.0,
            1.0,
        ];
        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            full.pixels_per_point,
            clear,
            &prims,
            &mut full.textures_delta,
            capture,
            &self.window,
        );
        let again = full
            .viewport_output
            .get(&ViewportId::ROOT)
            .is_some_and(|v| v.repaint_delay.is_zero());
        if again {
            self.window.request_redraw();
        }
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    w.write_image_data(img.as_raw())
        .map_err(std::io::Error::other)?;
    w.finish().map_err(std::io::Error::other)
}

/// Decodes a PNG (indexed or not) to RGBA for a texture.
pub fn decode_png_rgba(bytes: &[u8]) -> Option<([usize; 2], Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(([info.width as usize, info.height as usize], rgba))
}
