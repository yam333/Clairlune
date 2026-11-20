//! Which presentation reaches the screen from a window in the desktop layer?
//! Clears a live window on the last monitor to magenta through one wgpu
//! backend and keeps it up for a few seconds (sample the screen meanwhile).
//!
//! `cargo run -p clairlune-live --example present_probe -- dx12|vulkan|gl`

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    use clairlune_platform::windows_live::{DesktopLayer, LiveWindow, pump_messages_until};
    use clairlune_platform::{detect, init_dpi_awareness};
    use raw_window_handle::{HasWindowHandle, RawDisplayHandle, WindowsDisplayHandle};
    use std::time::{Duration, Instant};

    let backend = std::env::args().nth(1).unwrap_or_else(|| "dx12".into());
    let backends = match backend.as_str() {
        "vulkan" => wgpu::Backends::VULKAN,
        "gl" => wgpu::Backends::GL,
        _ => wgpu::Backends::DX12,
    };
    init_dpi_awareness();
    let host = detect().expect("host");
    let monitor = host
        .monitors()
        .expect("monitors")
        .into_iter()
        .last()
        .expect("a monitor");
    let layer = DesktopLayer::find().expect("desktop layer");
    let window = LiveWindow::create(&layer, &monitor).expect("window");
    window.show();
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let raw_window_handle = window.window_handle().expect("handle").as_raw();
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
            raw_window_handle,
        })
    }
    .expect("surface");
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("adapter");
    println!("{backend}: adapter {}", adapter.get_info().name);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("device");
    let caps = surface.get_capabilities(&adapter);
    println!("formats {:?} alpha {:?}", caps.formats, caps.alpha_modes);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .unwrap_or(caps.formats[0]);
    surface.configure(
        &device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: monitor.width,
            height: monitor.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: Default::default(),
        },
    );
    let end = Instant::now() + Duration::from_secs(4);
    while Instant::now() < end {
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => {
                println!("no frame: {other:?}");
                break;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        queue.submit([enc.finish()]);
        queue.present(frame);
        pump_messages_until(Instant::now() + Duration::from_millis(100));
    }
    println!("{backend}: done");
}
