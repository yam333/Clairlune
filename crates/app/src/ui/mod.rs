//! UI windows on the app's single event loop. A window
//! and everything it needs (egui context, GPU device, surface) are created
//! when it opens and dropped when it closes, so nothing of the UI is held
//! while the app sits in the tray.

pub mod archive_view;
pub mod glass;
pub mod library_view;
pub mod style;
pub mod timeline_view;
pub mod view;

use crate::library::Item;
use crate::settings::Settings;
use clairlune_scene::{APP_NAME, Theme};
use egui::ViewportId;
use std::path::Path;
use std::sync::Arc;
use view::{Status, UiAction, ViewState};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

/// Which window this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Settings,
    Library,
    Timeline,
    Archive,
}

/// What the windows show, borrowed from the app for one frame.
pub struct Model<'a> {
    pub settings: &'a mut Settings,
    pub status: &'a Status,
    pub library: &'a [Item],
    pub library_dir: &'a Path,
    /// Where the timeline uses each library item ("Monday 07:00").
    pub uses: &'a std::collections::BTreeMap<String, Vec<String>>,
    /// The daily timeline, edited in place by the Timeline window.
    pub timeline: &'a mut crate::timeline::Timeline,
    /// The archive's entries (oldest first), its folder, and the entry on
    /// screen.
    pub archive: &'a [crate::archive::Entry],
    pub archive_dir: &'a Path,
    pub current_entry: Option<&'a str>,
}

/// Library thumbnails as textures, each loaded once per window.
#[derive(Default)]
pub struct Thumbs(std::collections::HashMap<String, Option<egui::TextureHandle>>);

impl Thumbs {
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        library_dir: &Path,
        id: &str,
    ) -> Option<&egui::TextureHandle> {
        self.0
            .entry(id.to_string())
            .or_insert_with(|| {
                let bytes = std::fs::read(library_dir.join(id).join("thumb.png")).ok()?;
                let (size, rgba) = decode_png_rgba(&bytes)?;
                let img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                Some(ctx.load_texture(id, img, egui::TextureOptions::LINEAR))
            })
            .as_ref()
    }
}

pub struct UiWindow {
    kind: Kind,
    library: library_view::LibraryState,
    timeline: timeline_view::TimelineState,
    archive: archive_view::ArchiveState,
    window: Arc<Window>,
    state: egui_winit::State,
    painter: egui_wgpu::winit::Painter,
    view: ViewState,
    theme: Theme,
    /// Save the next frame to this PNG (UI review screenshots).
    pub screenshot_to: Option<std::path::PathBuf>,
    /// When egui wants its next frame (an animation running), if not now.
    repaint_at: Option<std::time::Instant>,
}

/// The graphics API the system is built around.
fn native_backends() -> wgpu::Backends {
    if cfg!(windows) {
        wgpu::Backends::DX12
    } else if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    }
}

/// The PCI ids of the GPU driving the primary monitor (Windows).
fn display_gpu() -> Option<(u32, u32)> {
    #[cfg(windows)]
    {
        let host = clairlune_platform::detect().ok()?;
        let monitors = host.monitors().ok()?;
        let primary: Vec<_> = monitors.into_iter().filter(|m| m.primary).collect();
        clairlune_platform::windows_live::display_adapters(&primary)
            .into_iter()
            .flatten()
            .next()
            .map(|a| (a.vendor, a.device))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// What a window event means for the app.
pub enum Outcome {
    Nothing,
    Close,
}

impl UiWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        kind: Kind,
        theme: Theme,
        section: view::Section,
    ) -> Result<UiWindow, String> {
        // The smallest sizes at which every page still fits without
        // anything cut off (the sidebar, a row's label and its controls).
        let (title, w, h, min) = match kind {
            Kind::Settings => (
                format!("{APP_NAME} Settings: {}", section.name()),
                1000.0_f64.max(f64::from(view::SETTINGS_MIN_WIDTH)),
                720.0,
                (f64::from(view::SETTINGS_MIN_WIDTH), 640.0),
            ),
            Kind::Library => (format!("{APP_NAME} Library"), 980.0, 660.0, (820.0, 540.0)),
            Kind::Archive => (format!("{APP_NAME} Archive"), 980.0, 660.0, (820.0, 540.0)),
            Kind::Timeline => (
                format!("{APP_NAME} Timeline"),
                1280.0,
                720.0,
                (980.0, 560.0),
            ),
        };
        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(w, h))
            .with_min_inner_size(winit::dpi::LogicalSize::new(min.0, min.1))
            .with_enabled_buttons(winit::window::WindowButtons::CLOSE)
            // In the title bar and the taskbar button.
            .with_window_icon(
                winit::window::Icon::from_rgba(crate::icon::app_icon_rgba(32), 32, 32).ok(),
            )
            .with_theme(Some(match theme {
                Theme::Light => winit::window::Theme::Light,
                Theme::Dark => winit::window::Theme::Dark,
            }))
            .with_visible(false);
        let attrs = if kind == Kind::Settings {
            glass_frame(attrs)
        } else {
            attrs
        };
        let window = Arc::new(event_loop.create_window(attrs).map_err(|e| e.to_string())?);
        let ctx = egui::Context::default();
        ctx.set_fonts(style::fonts());
        style::apply(&ctx, theme);
        let mut config = egui_wgpu::WgpuConfiguration::default();
        if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup {
            // Only the system's own graphics API: probing every API on every
            // GPU loads each driver into the process (~370 MB that closing
            // the window never gave back).
            setup.instance_descriptor.backends = native_backends();
            // The GPU the monitors are plugged into, which live playback
            // already uses, so no second driver loads; else a low-power one.
            setup.power_preference = wgpu::PowerPreference::LowPower;
            if let Some((vendor, device)) = display_gpu() {
                setup.native_adapter_selector = Some(Arc::new(move |adapters, surface| {
                    adapters
                        .iter()
                        .find(|a| {
                            let i = a.get_info();
                            i.vendor == vendor
                                && i.device == device
                                && surface.is_none_or(|s| a.is_surface_supported(s))
                        })
                        .or_else(|| adapters.first())
                        .cloned()
                        .ok_or_else(|| "no graphics adapter".to_string())
                }));
            }
            setup.device_descriptor = Arc::new(|adapter| wgpu::DeviceDescriptor {
                label: Some("clairlune-ui"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            });
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
        Ok(UiWindow {
            kind,
            library: library_view::LibraryState::default(),
            timeline: timeline_view::TimelineState::default(),
            archive: archive_view::ArchiveState::default(),
            window,
            state,
            painter,
            view: ViewState {
                section,
                thumb: None,
                toast: None,
                page: Default::default(),
                backdrop: None,
                grain: None,
                chrome: None,
                lands: Default::default(),
            },
            theme,
            screenshot_to: None,
            repaint_at: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Selects a library item in the Library window.
    pub fn select_item(&mut self, item: &Item) {
        self.library.select(item);
        self.window.request_redraw();
    }

    /// Selects an entry in the Archive window.
    pub fn select_entry(&mut self, name: &str) {
        self.archive.select(name);
        self.window.request_redraw();
    }

    /// Selects a slot in the Timeline window.
    pub fn select_slot(&mut self, day: usize, index: usize) {
        self.timeline.select(day, index);
        self.window.request_redraw();
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

    /// Lays the glass over this image of the wallpaper on screen (RGBA).
    pub fn set_backdrop(&mut self, size: [usize; 2], rgba: &[u8]) {
        self.view.backdrop = glass::Backdrop::new(self.state.egui_ctx(), size, rgba);
        self.window.request_redraw();
    }

    /// Where Content's landscape tiles are drawn to.
    pub fn set_land_dir(&mut self, dir: std::path::PathBuf) {
        self.view.lands.dir = Some(dir);
        self.view.lands.refresh();
        self.window.request_redraw();
    }

    /// More landscape tiles have been drawn: show them.
    pub fn refresh_lands(&mut self) {
        self.view.lands.refresh();
        self.window.request_redraw();
    }

    pub fn set_section(&mut self, section: view::Section) {
        self.view.section = section;
        self.window.request_redraw();
    }

    /// Asks for the frame egui wanted by now; returns when the next one is
    /// due, for the event loop to wake then.
    pub fn redraw_due(&mut self, now: std::time::Instant) -> Option<std::time::Instant> {
        match self.repaint_at {
            Some(at) if at <= now => {
                self.repaint_at = None;
                self.window.request_redraw();
                None
            }
            later => later,
        }
    }

    /// The window itself (a dialog's parent).
    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    /// Brings the window forward (restoring it when minimized).
    pub fn focus(&self) {
        self.window.set_minimized(false);
        self.window.focus_window();
    }

    /// The page's own state (UI test: a monitor picked, details shown,
    /// scrolled down).
    pub fn page_state(&mut self) -> &mut view::PageState {
        self.window.request_redraw();
        &mut self.view.page
    }

    /// Shows a brief note over the page (UI test).
    pub fn toast(&mut self, text: &str) {
        self.view.toast = Some((text.to_string(), std::time::Instant::now()));
        self.window.request_redraw();
    }

    pub fn section(&self) -> view::Section {
        self.view.section
    }

    pub fn clear_thumb(&mut self) {
        self.view.thumb = None;
        self.window.request_redraw();
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Handles one window event; paints on redraw and collects the user's actions.
    pub fn on_event(
        &mut self,
        event: &WindowEvent,
        model: &mut Model,
        actions: &mut Vec<UiAction>,
    ) -> Outcome {
        match event {
            WindowEvent::CloseRequested => return Outcome::Close,
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (
                    std::num::NonZeroU32::new(size.width),
                    std::num::NonZeroU32::new(size.height),
                ) {
                    self.painter.on_window_resized(ViewportId::ROOT, w, h);
                }
            }
            WindowEvent::RedrawRequested => {
                return if self.paint(model, actions) {
                    Outcome::Close
                } else {
                    Outcome::Nothing
                };
            }
            _ => {}
        }
        let resp = self.state.on_window_event(&self.window, event);
        if resp.repaint {
            self.window.request_redraw();
        }
        Outcome::Nothing
    }

    /// Paints a frame; true when the window's own close button was pressed.
    fn paint(&mut self, model: &mut Model, actions: &mut Vec<UiAction>) -> bool {
        let input = self.state.take_egui_input(&self.window);
        let ctx = self.state.egui_ctx().clone();
        let shown = self.view.section;
        let theme = self.theme;
        let kind = self.kind;
        let view = &mut self.view;
        let library = &mut self.library;
        let timeline = &mut self.timeline;
        let archive = &mut self.archive;
        let mut full = ctx.run_ui(input, |ui| match kind {
            Kind::Settings => view::show(ui, theme, view, model.settings, model.status, actions),
            Kind::Library => library_view::show(ui, theme, library, model, actions),
            Kind::Timeline => timeline_view::show(ui, theme, timeline, model, actions),
            Kind::Archive => archive_view::show(
                ui,
                theme,
                archive,
                &archive_view::ArchiveModel {
                    entries: model.archive,
                    dir: model.archive_dir,
                    current: model.current_entry,
                    keep_days: model.settings.archive.keep_days,
                    max_mb: model.settings.archive.max_mb,
                    status: model.status,
                },
                actions,
            ),
        });
        // What the window's own frame asked for (Windows).
        let mut close = false;
        match self.view.chrome.take() {
            Some(view::Chrome::Drag) => {
                let _ = self.window.drag_window();
            }
            Some(view::Chrome::Resize(d)) => {
                let _ = self.window.drag_resize_window(d);
            }
            Some(view::Chrome::Minimize) => self.window.set_minimized(true),
            Some(view::Chrome::Close) => close = true,
            None => {}
        }
        if kind == Kind::Settings && self.view.section != shown {
            // The title names the page (macOS HIG; harmless elsewhere).
            self.window.set_title(&format!(
                "{APP_NAME} Settings: {}",
                self.view.section.name()
            ));
        }
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
        // A frame now, or one later (embers, a drifting name): the event
        // loop wakes for it (`redraw_due`), or animations would only move
        // while the pointer does.
        let delay = full
            .viewport_output
            .get(&ViewportId::ROOT)
            .map(|v| v.repaint_delay);
        self.repaint_at = None;
        match delay {
            Some(d) if d.is_zero() => self.window.request_redraw(),
            Some(d) if d < std::time::Duration::from_secs(3600) => {
                self.repaint_at = Some(std::time::Instant::now() + d);
            }
            _ => {}
        }
        close
    }
}

/// Settings' window as glass: on Windows without the system's frame (the
/// view draws its own buttons), keeping the shadow and the rounded corners;
/// on macOS the title bar turns see-through over the glass, its buttons
/// staying where they are.
fn glass_frame(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    #[cfg(windows)]
    {
        use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
        attrs
            .with_decorations(false)
            .with_undecorated_shadow(true)
            .with_corner_preference(CornerPreference::Round)
    }
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attrs
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        attrs
    }
}

impl Drop for UiWindow {
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
