//! The live wallpaper on Windows: one window per monitor in the desktop layer
//! (behind the icons), each showing a [`Player`].
//!
//! Frames go through the GPU (DX12): on the raised desktop of Windows 11 24H2
//! and later, only DirectX presents of such a window reach the screen; GDI
//! drawing doesn't (see the M7 findings). Without a GPU device the frames go
//! through softbuffer (GDI), which shows on the classic desktop. Either way
//! only the rectangles that changed are copied.

use crate::Player;
use crate::gpu::{Canvas, Gpu, exact_format};
use clairlune_platform::Monitor;
use clairlune_platform::windows_live::{
    DesktopLayer, DisplayAdapter, LiveWindow, Win32Display, covered, display_adapters, take_repaint,
};
use raw_window_handle::{HasWindowHandle, RawDisplayHandle, WindowsDisplayHandle};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

// One per monitor: the size difference between the variants doesn't matter.
#[allow(clippy::large_enum_variant)]
enum Present {
    Gpu {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        canvas: Canvas,
    },
    Soft(softbuffer::Surface<Win32Display, LiveWindow>),
}

struct Output {
    monitor: Monitor,
    player: Player,
    /// A window covers the whole monitor: paused there.
    covered: bool,
    /// Declared before `window`: the surface must go first.
    present: Present,
    /// The window (GPU path; softbuffer owns its own).
    window: Option<LiveWindow>,
}

/// What one frame cost, for budgets and logs.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    /// Pixels uploaded or copied, over all monitors.
    pub pixels: u64,
    /// Rectangles uploaded or copied.
    pub rects: usize,
    /// Monitors paused because a window covers them.
    pub covered: usize,
    /// Presents that waited over 50 ms for the swapchain (the compositor
    /// holding frames back).
    pub stalls: usize,
    /// Time spent compositing, uploading and presenting.
    pub busy: Duration,
    /// Of which uploading, and drawing plus presenting.
    pub upload: Duration,
    pub present: Duration,
}

/// Live wallpapers on every monitor given.
pub struct Desktop {
    layer: DesktopLayer,
    gpu: Option<(wgpu::Instance, Gpu)>,
    soft: Option<softbuffer::Context<Win32Display>>,
    outputs: Vec<Output>,
    /// When the monitors were last checked for covering windows.
    checked: Option<Instant>,
    /// The adapter driving most of the monitors: frames are drawn there.
    prefer: Option<DisplayAdapter>,
}

/// How often to look for windows covering a monitor.
const COVER_CHECK: Duration = Duration::from_millis(500);

fn nz(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v.max(1)).unwrap_or(NonZeroU32::MIN)
}

fn surface_for(
    instance: &wgpu::Instance,
    window: &LiveWindow,
) -> Result<wgpu::Surface<'static>, String> {
    let raw_window_handle = window.window_handle().map_err(|e| e.to_string())?.as_raw();
    // The surface is dropped before the window (field order in `Output`).
    unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
            raw_window_handle,
        })
    }
    .map_err(|e| e.to_string())
}

impl Desktop {
    /// Puts each player on its monitor (the player's scene must be the
    /// monitor's size) and shows its first frame.
    pub fn start(scenes: Vec<(Monitor, Player)>) -> Result<Desktop, String> {
        let layer = DesktopLayer::find().map_err(|e| e.to_string())?;
        let monitors: Vec<Monitor> = scenes.iter().map(|(m, _)| m.clone()).collect();
        let mut counts: Vec<(DisplayAdapter, usize)> = Vec::new();
        for a in display_adapters(&monitors).into_iter().flatten() {
            match counts.iter_mut().find(|(b, _)| *b == a) {
                Some((_, n)) => *n += 1,
                None => counts.push((a, 1)),
            }
        }
        let prefer = counts.into_iter().max_by_key(|(_, n)| *n).map(|(a, _)| a);
        let mut desktop = Desktop {
            layer,
            gpu: None,
            soft: None,
            outputs: Vec::new(),
            checked: None,
            prefer,
        };
        for (monitor, player) in scenes {
            desktop.add(monitor, player)?;
        }
        tracing::info!(
            target: "clairlune",
            layout = desktop.layer.layout(),
            monitors = desktop.outputs.len(),
            presenter = %desktop.presenter(),
            "live wallpaper started"
        );
        Ok(desktop)
    }

    /// "gpu (adapter)" or "gdi".
    pub fn presenter(&self) -> String {
        match &self.gpu {
            Some((_, g)) => format!("gpu ({})", g.adapter.get_info().name),
            None => "gdi".into(),
        }
    }

    fn add(&mut self, monitor: Monitor, mut player: Player) -> Result<(), String> {
        if player.size() != (monitor.width, monitor.height) {
            return Err(format!(
                "the scene is {:?} but monitor {} is {}x{}",
                player.size(),
                monitor.id,
                monitor.width,
                monitor.height
            ));
        }
        let window = LiveWindow::create(&self.layer, &monitor).map_err(|e| e.to_string())?;
        window.show();
        let t = player.now_ms();
        player.advance(t);
        if self.soft.is_none() {
            match self.add_gpu(&monitor, &window, &player) {
                Ok(present) => {
                    self.outputs.push(Output {
                        monitor,
                        player,
                        covered: false,
                        present,
                        window: Some(window),
                    });
                    return Ok(());
                }
                // GDI frames don't show on the raised desktop: keep the still.
                Err(e) if self.gpu.is_none() && self.layer.layout() == "raised" => {
                    return Err(format!(
                        "live wallpapers need a GPU on this version of Windows: {e}"
                    ));
                }
                Err(e) if self.gpu.is_none() => {
                    tracing::warn!(target: "clairlune", error = %e, "no GPU for live wallpapers; using GDI");
                }
                Err(e) => return Err(e),
            }
        }
        if self.soft.is_none() {
            self.soft = Some(softbuffer::Context::new(Win32Display).map_err(|e| e.to_string())?);
        }
        let context = self.soft.as_ref().expect("just set");
        let mut surface = softbuffer::Surface::new(context, window).map_err(|e| e.to_string())?;
        surface
            .resize(nz(monitor.width), nz(monitor.height))
            .map_err(|e| e.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|e| e.to_string())?;
        player.write_all(&mut buffer);
        buffer.present().map_err(|e| e.to_string())?;
        self.outputs.push(Output {
            monitor,
            player,
            covered: false,
            present: Present::Soft(surface),
            window: None,
        });
        Ok(())
    }

    fn add_gpu(
        &mut self,
        monitor: &Monitor,
        window: &LiveWindow,
        player: &Player,
    ) -> Result<Present, String> {
        if self.gpu.is_none() {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = wgpu::Backends::DX12;
            let instance = wgpu::Instance::new(desc);
            let surface = surface_for(&instance, window)?;
            let prefer = self.prefer.as_ref().map(|a| (a.vendor, a.device));
            let gpu = Gpu::new(&instance, Some(&surface), prefer, false)?;
            tracing::info!(
                target: "clairlune",
                adapter = %gpu.adapter.get_info().name,
                memory_mb = clairlune_platform::windows_live::process_memory() >> 20,
                "live: GPU device ready"
            );
            self.gpu = Some((instance, gpu));
            return self.configure(monitor, surface, player);
        }
        let (instance, _) = self.gpu.as_ref().expect("checked");
        let surface = surface_for(instance, window)?;
        self.configure(monitor, surface, player)
    }

    fn configure(
        &mut self,
        monitor: &Monitor,
        surface: wgpu::Surface<'static>,
        player: &Player,
    ) -> Result<Present, String> {
        let (_, gpu) = self.gpu.as_mut().expect("set up");
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = exact_format(&caps.formats)
            .ok_or_else(|| format!("no 8-bit surface format in {:?}", caps.formats))?;
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto)
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: monitor.width,
            height: monitor.height,
            // DWM composes these windows, so nothing tears; a mode that
            // doesn't wait for each monitor's vblank in turn keeps the frame
            // short.
            present_mode: [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
                .into_iter()
                .find(|m| caps.present_modes.contains(m))
                .unwrap_or(wgpu::PresentMode::Fifo),
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
            color_space: Default::default(),
        };
        surface.configure(&gpu.device, &config);
        let canvas = gpu.canvas(player);
        tracing::info!(
            target: "clairlune",
            monitor = %monitor.id,
            mode = ?config.present_mode,
            memory_mb = clairlune_platform::windows_live::process_memory() >> 20,
            "live: surface ready"
        );
        let mut present = Present::Gpu {
            surface,
            config,
            canvas,
        };
        let _ = Self::draw_gpu(gpu, &mut present);
        Ok(present)
    }

    /// Draws a GPU output's canvas and presents it; returns how long getting
    /// the swapchain image took.
    fn draw_gpu(gpu: &mut Gpu, present: &mut Present) -> Duration {
        let Present::Gpu {
            surface,
            config,
            canvas,
        } = present
        else {
            return Duration::ZERO;
        };
        let asked = Instant::now();
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                surface.configure(&gpu.device, config);
                return asked.elapsed();
            }
            _ => return asked.elapsed(), // timed out or occluded: try next tick
        };
        let waited = asked.elapsed();
        let view = frame.texture.create_view(&Default::default());
        gpu.draw(canvas, &view, config.format);
        gpu.queue.present(frame);
        waited
    }

    /// The desktop layer and every window are still in place. False after
    /// Explorer restarts: stop and start again.
    pub fn healthy(&self) -> bool {
        self.layer.is_alive()
            && self.outputs.iter().all(|o| match (&o.window, &o.present) {
                (Some(w), _) => w.is_attached(),
                (None, Present::Soft(s)) => s.window().is_attached(),
                (None, Present::Gpu { .. }) => false,
            })
    }

    /// The monitors playing.
    pub fn monitors(&self) -> impl Iterator<Item = &Monitor> {
        self.outputs.iter().map(|o| &o.monitor)
    }

    /// Draws the current time on every monitor.
    pub fn tick(&mut self) -> Result<FrameStats, String> {
        let started = Instant::now();
        let whole = take_repaint();
        let mut pixels = 0u64;
        let (mut upload, mut present, mut count) = (Duration::ZERO, Duration::ZERO, 0usize);
        let mut stalls = 0usize;
        if self.checked.is_none_or(|t| t.elapsed() >= COVER_CHECK) {
            self.checked = Some(Instant::now());
            let monitors: Vec<Monitor> = self.outputs.iter().map(|o| o.monitor.clone()).collect();
            for (o, c) in self.outputs.iter_mut().zip(covered(&monitors)) {
                if o.covered != c {
                    tracing::debug!(target: "clairlune", monitor = %o.monitor.id, covered = c, "live: monitor covered");
                }
                o.covered = c;
            }
        }
        let paused = self.outputs.iter().filter(|o| o.covered).count();
        for o in &mut self.outputs {
            if o.covered {
                continue; // nobody sees it; resumes at the time it is then
            }
            let t = o.player.now_ms();
            let rects = o.player.advance(t);
            let area = rects.iter().map(|r| u64::from(r.w * r.h)).sum::<u64>();
            match &mut o.present {
                Present::Gpu { canvas, .. } => {
                    let Some((_, gpu)) = self.gpu.as_mut() else {
                        continue;
                    };
                    let canvas: &mut Canvas = canvas;
                    if rects.is_empty() && !whole {
                        continue; // the last frame stays on screen
                    }
                    let t0 = Instant::now();
                    gpu.upload(canvas, &o.player, &rects);
                    let t1 = Instant::now();
                    pixels += area;
                    count += rects.len();
                    let waited = Self::draw_gpu(gpu, &mut o.present);
                    if waited > Duration::from_millis(50) {
                        stalls += 1;
                        tracing::debug!(target: "clairlune", monitor = %o.monitor.id, ms = waited.as_millis() as u64, "live: present waited");
                    }
                    upload += t1 - t0;
                    present += t1.elapsed();
                }
                Present::Soft(surface) => {
                    let mut buffer = surface.buffer_mut().map_err(|e| e.to_string())?;
                    if whole || buffer.age() == 0 {
                        o.player.write_all(&mut buffer);
                        pixels += u64::from(o.monitor.width) * u64::from(o.monitor.height);
                        buffer.present().map_err(|e| e.to_string())?;
                        continue;
                    }
                    if rects.is_empty() {
                        continue;
                    }
                    o.player.write(&mut buffer, &rects);
                    let damage: Vec<softbuffer::Rect> = rects
                        .iter()
                        .map(|r| softbuffer::Rect {
                            x: r.x,
                            y: r.y,
                            width: nz(r.w),
                            height: nz(r.h),
                        })
                        .collect();
                    pixels += area;
                    buffer
                        .present_with_damage(&damage)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(FrameStats {
            pixels,
            rects: count,
            covered: paused,
            stalls,
            busy: started.elapsed(),
            upload,
            present,
        })
    }
}
