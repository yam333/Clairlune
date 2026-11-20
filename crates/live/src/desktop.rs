//! The live wallpaper on Windows: one window per monitor in the desktop layer
//! (behind the icons), each showing a [`Player`].
//!
//! Frames go through Direct3D 11 with a flip-sequential swapchain: only the
//! rectangles that changed are copied and presented as dirty rects, so the
//! compositor recomposes just those (see `d3d11`). On the raised desktop of
//! Windows 11 24H2 and later only DirectX presents of such a window reach the
//! screen; GDI drawing doesn't (see the M7 findings). If Direct3D 11 can't
//! start, wgpu (DirectX 12) draws whole frames; without any GPU device,
//! softbuffer (GDI) copies the changed rectangles on the classic desktop.

use crate::Player;
use crate::d3d11::{D3d11, FlipSurface};
use crate::gpu::{Canvas, Gpu, exact_format};
use clairlune_platform::Monitor;
use clairlune_platform::windows_live::{
    DesktopLayer, DisplayAdapter, LiveWindow, Win32Display, covered, display_adapters,
    displays_off, session_locked, take_repaint,
};
use raw_window_handle::{HasWindowHandle, RawDisplayHandle, WindowsDisplayHandle};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

/// The GDI fallback's frame-rate cap (Balanced).
pub const GDI_MAX_FPS: u32 = 12;

// One per monitor: the size difference between the variants doesn't matter.
#[allow(clippy::large_enum_variant)]
enum Present {
    Flip(FlipSurface),
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

pub use crate::{CoverPolicy, FrameStats};

/// Live wallpapers on every monitor given.
pub struct Desktop {
    layer: DesktopLayer,
    /// The light presenter, and whether it failed to start (then wgpu).
    d3d: Option<D3d11>,
    d3d_failed: bool,
    gpu: Option<(wgpu::Instance, Gpu)>,
    soft: Option<softbuffer::Context<Win32Display>>,
    outputs: Vec<Output>,
    /// When the monitors were last checked for covering windows.
    checked: Option<Instant>,
    /// The adapter driving most of the monitors: frames are drawn there.
    prefer: Option<DisplayAdapter>,
    /// The session is locked or the displays are off: nothing is drawn.
    away: bool,
    /// What covering windows pause.
    pub cover_policy: CoverPolicy,
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
            d3d: None,
            d3d_failed: false,
            gpu: None,
            soft: None,
            outputs: Vec::new(),
            checked: None,
            prefer,
            away: false,
            cover_policy: CoverPolicy::default(),
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

    /// "raised" (Windows 11 24H2+) or "classic".
    pub fn layout(&self) -> &'static str {
        self.layer.layout()
    }

    /// The highest frame rate worth asking for: GDI copies every changed
    /// pixel on the CPU, so without a GPU live plays at most at Balanced.
    pub fn max_fps(&self) -> u32 {
        if self.d3d.is_none() && self.gpu.is_none() {
            GDI_MAX_FPS
        } else {
            60
        }
    }

    /// "d3d11 (adapter)", "gpu (adapter)" or "gdi".
    pub fn presenter(&self) -> String {
        if let Some(d) = &self.d3d {
            return format!("d3d11 ({})", d.name);
        }
        match &self.gpu {
            Some((_, g)) => format!("gpu ({})", g.adapter.get_info().name),
            None => "gdi".into(),
        }
    }

    /// The Direct3D 11 swapchain for a window, showing the player's frame;
    /// `None` when Direct3D 11 isn't available here (it then isn't tried
    /// again).
    fn add_flip(
        &mut self,
        monitor: &Monitor,
        window: &LiveWindow,
        player: &Player,
    ) -> Option<Present> {
        // Diagnostics: CLAIRLUNE_PRESENTER=wgpu skips Direct3D 11.
        if self.d3d_failed || std::env::var("CLAIRLUNE_PRESENTER").as_deref() == Ok("wgpu") {
            return None;
        }
        if self.d3d.is_none() {
            let prefer = self.prefer.as_ref().map(|a| (a.vendor, a.device));
            match D3d11::new(prefer) {
                Ok(d) => {
                    tracing::info!(
                        target: "clairlune",
                        adapter = %d.name,
                        memory_mb = clairlune_platform::windows_live::process_memory() >> 20,
                        "live: Direct3D 11 ready"
                    );
                    self.d3d = Some(d);
                }
                Err(e) => {
                    tracing::warn!(target: "clairlune", error = %e, "no Direct3D 11; trying DirectX 12");
                    self.d3d_failed = true;
                    return None;
                }
            }
        }
        let d3d = self.d3d.as_ref()?;
        let mut surface = match d3d.surface(window.hwnd(), monitor.width, monitor.height) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "no Direct3D 11 swapchain; trying DirectX 12");
                return None;
            }
        };
        if let Err(e) = d3d.present(&mut surface, player, &[], true) {
            tracing::warn!(target: "clairlune", error = %e, "Direct3D 11 present failed; trying DirectX 12");
            return None;
        }
        Some(Present::Flip(surface))
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
        // Smooth particles on the GPU paths; GDI (below) keeps the grid.
        player.set_smooth(self.soft.is_none());
        let t = player.now_ms();
        player.advance(t);
        if self.soft.is_none()
            && self.gpu.is_none()
            && let Some(present) = self.add_flip(&monitor, &window, &player)
        {
            self.outputs.push(Output {
                monitor,
                player,
                covered: false,
                present,
                window: Some(window),
            });
            return Ok(());
        }
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
        player.set_smooth(false);
        let t = player.now_ms();
        player.advance(t);
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
                (None, Present::Gpu { .. } | Present::Flip(_)) => false,
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
            let away = session_locked() || displays_off();
            if away != self.away {
                tracing::info!(target: "clairlune", away, "live: locked or displays off");
                self.away = away;
            }
            let monitors: Vec<Monitor> = self.outputs.iter().map(|o| o.monitor.clone()).collect();
            let mut found = match self.cover_policy {
                CoverPolicy::Off => vec![false; monitors.len()],
                _ => covered(&monitors),
            };
            if self.cover_policy == CoverPolicy::AllMonitors && found.iter().any(|&c| c) {
                found.iter_mut().for_each(|c| *c = true);
            }
            for (o, c) in self.outputs.iter_mut().zip(found) {
                if o.covered != c {
                    tracing::debug!(target: "clairlune", monitor = %o.monitor.id, covered = c, "live: monitor covered");
                }
                o.covered = c;
            }
        }
        let paused = self.outputs.iter().filter(|o| o.covered).count();
        for o in &mut self.outputs {
            if o.covered || self.away {
                continue; // nobody sees it; resumes at the time it is then
            }
            let t = o.player.now_ms();
            let rects = o.player.advance(t);
            let area = rects.iter().map(|r| u64::from(r.w * r.h)).sum::<u64>();
            match &mut o.present {
                Present::Flip(surface) => {
                    let Some(d3d) = self.d3d.as_ref() else {
                        continue;
                    };
                    let t0 = Instant::now();
                    pixels += d3d.present(surface, &o.player, &rects, whole)?;
                    count += rects.len();
                    present += t0.elapsed();
                }
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
            away: self.away,
            stalls,
            busy: started.elapsed(),
            upload,
            present,
        })
    }
}
