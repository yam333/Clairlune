//! The live wallpaper on macOS: one borderless window per screen at the
//! desktop picture's window level (above the wallpaper, below the desktop
//! icons), on every Space, ignoring the mouse, drawn with wgpu on Metal.
//! Only the rectangles that changed are uploaded; the Metal layer presents
//! the frame.
//!
//! A screen whose window is fully covered (a zoomed app, the lock screen, a
//! sleeping display) reports itself occluded, and isn't drawn until it shows
//! again.
//!
//! Untried: written and compile-checked without a Mac. Everything here runs
//! on the main thread (AppKit's rule), which is where the app ticks it.

use crate::gpu::{Canvas, Gpu, exact_format};
use crate::{CoverPolicy, FrameStats, Player};
use clairlune_platform::Monitor;
use clairlune_platform::macos::find_screen;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSWindow, NSWindowCollectionBehavior, NSWindowOcclusionState,
    NSWindowStyleMask,
};
use objc2_core_graphics::{CGWindowLevelForKey, CGWindowLevelKey};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use std::ptr::NonNull;
use std::time::{Duration, Instant};

/// One screen's window and what draws into it. The surface is dropped
/// before the window (field order).
struct Output {
    monitor: Monitor,
    player: Player,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    canvas: Canvas,
    covered: bool,
    window: Retained<NSWindow>,
}

/// Live wallpapers on every screen given.
pub struct MacDesktop {
    outputs: Vec<Output>,
    gpu: Gpu,
    _instance: wgpu::Instance,
    pub cover_policy: CoverPolicy,
}

/// A borderless window covering the screen, at the desktop picture's level.
fn desktop_window(mtm: MainThreadMarker, monitor: &Monitor) -> Result<Retained<NSWindow>, String> {
    let screen = find_screen(mtm, &monitor.id)
        .ok_or_else(|| format!("screen {} isn't connected", monitor.id))?;
    let frame = screen.frame();
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // Owned by the Retained handle, not released again on close.
    unsafe { window.setReleasedWhenClosed(false) };
    let level = CGWindowLevelForKey(CGWindowLevelKey::DesktopWindowLevelKey);
    window.setLevel(level as isize);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    window.setIgnoresMouseEvents(true);
    window.setHasShadow(false);
    window.setOpaque(true);
    window.setFrame_display(frame, true);
    window.orderFrontRegardless();
    Ok(window)
}

fn surface_for(
    instance: &wgpu::Instance,
    window: &NSWindow,
) -> Result<wgpu::Surface<'static>, String> {
    let view = window.contentView().ok_or("the window has no view")?;
    let handle = AppKitWindowHandle::new(NonNull::from(&*view).cast());
    // The surface is dropped before the window (field order in `Output`),
    // and the window keeps its view.
    unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AppKit(handle),
        })
    }
    .map_err(|e| e.to_string())
}

impl MacDesktop {
    /// Puts each player on its screen (the player's scene must be the
    /// screen's size in pixels) and shows its first frame. Main thread only.
    pub fn start(scenes: Vec<(Monitor, Player)>) -> Result<MacDesktop, String> {
        let mtm = MainThreadMarker::new().ok_or("live wallpapers start on the main thread")?;
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(desc);
        let mut gpu: Option<Gpu> = None;
        let mut outputs = Vec::new();
        for (monitor, mut player) in scenes {
            if player.size() != (monitor.width, monitor.height) {
                return Err(format!(
                    "the scene is {:?} but screen {} is {}x{}",
                    player.size(),
                    monitor.id,
                    monitor.width,
                    monitor.height
                ));
            }
            let window = desktop_window(mtm, &monitor)?;
            let surface = surface_for(&instance, &window)?;
            if gpu.is_none() {
                let g = Gpu::new(&instance, Some(&surface), None, false)?;
                tracing::info!(target: "clairlune", adapter = %g.adapter.get_info().name, "live: Metal device ready");
                gpu = Some(g);
            }
            let g = gpu.as_mut().expect("just set");
            let caps = surface.get_capabilities(&g.adapter);
            let format = exact_format(&caps.formats)
                .ok_or_else(|| format!("no 8-bit surface format in {:?}", caps.formats))?;
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: monitor.width,
                height: monitor.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                    wgpu::CompositeAlphaMode::Opaque
                } else {
                    wgpu::CompositeAlphaMode::Auto
                },
                view_formats: vec![],
                color_space: Default::default(),
            };
            surface.configure(&g.device, &config);
            let t = player.now_ms();
            player.advance(t);
            let canvas = g.canvas(&player);
            let mut out = Output {
                monitor,
                player,
                surface,
                config,
                canvas,
                covered: false,
                window,
            };
            draw(g, &mut out);
            outputs.push(out);
        }
        let gpu = gpu.ok_or("no screens to play on")?;
        Ok(MacDesktop {
            outputs,
            gpu,
            _instance: instance,
            cover_policy: CoverPolicy::default(),
        })
    }

    /// "metal (adapter)".
    pub fn presenter(&self) -> String {
        format!("metal ({})", self.gpu.adapter.get_info().name)
    }

    /// The highest frame rate worth asking for.
    pub fn max_fps(&self) -> u32 {
        60
    }

    /// The screens playing.
    pub fn monitors(&self) -> impl Iterator<Item = &Monitor> {
        self.outputs.iter().map(|o| &o.monitor)
    }

    /// Draws the current time on every screen that shows.
    pub fn tick(&mut self) -> Result<FrameStats, String> {
        let started = Instant::now();
        let shown: Vec<bool> = self
            .outputs
            .iter()
            .map(|o| {
                o.window
                    .occlusionState()
                    .contains(NSWindowOcclusionState::Visible)
            })
            .collect();
        let any_hidden = shown.iter().any(|v| !v);
        for (o, visible) in self.outputs.iter_mut().zip(&shown) {
            o.covered = match self.cover_policy {
                CoverPolicy::Off => false,
                CoverPolicy::ThatMonitor => !visible,
                CoverPolicy::AllMonitors => any_hidden,
            };
        }
        let (mut pixels, mut count) = (0u64, 0usize);
        let (mut upload, mut present) = (Duration::ZERO, Duration::ZERO);
        let mut stalls = 0;
        for o in &mut self.outputs {
            if o.covered {
                continue; // nobody sees it; resumes at the time it is then
            }
            let t = o.player.now_ms();
            let rects = o.player.advance(t);
            if rects.is_empty() {
                continue;
            }
            let t0 = Instant::now();
            self.gpu.upload(&mut o.canvas, &o.player, &rects);
            let t1 = Instant::now();
            pixels += rects.iter().map(|r| u64::from(r.w * r.h)).sum::<u64>();
            count += rects.len();
            if draw(&mut self.gpu, o) > Duration::from_millis(50) {
                stalls += 1;
            }
            upload += t1 - t0;
            present += t1.elapsed();
        }
        Ok(FrameStats {
            pixels,
            rects: count,
            covered: self.outputs.iter().filter(|o| o.covered).count(),
            away: false,
            stalls,
            busy: started.elapsed(),
            upload,
            present,
        })
    }
}

/// Draws an output's canvas and presents it; returns how long getting the
/// drawable took.
fn draw(gpu: &mut Gpu, o: &mut Output) -> Duration {
    let asked = Instant::now();
    let frame = match o.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            o.surface.configure(&gpu.device, &o.config);
            return asked.elapsed();
        }
        _ => return asked.elapsed(), // timed out or occluded: try next tick
    };
    let waited = asked.elapsed();
    let view = frame.texture.create_view(&Default::default());
    gpu.draw(&o.canvas, &view, o.config.format);
    gpu.queue.present(frame);
    waited
}

impl Drop for MacDesktop {
    fn drop(&mut self) {
        for o in &self.outputs {
            o.window.orderOut(None);
            o.window.close();
        }
    }
}
