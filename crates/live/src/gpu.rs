//! The GPU presenter: each monitor's frame lives on the
//! GPU as an index texture (one byte per pixel) with its 256-entry palette,
//! and one full-screen triangle looks each pixel up. Only the rectangles the
//! compositor reports are uploaded per frame.
//!
//! The lookup is exact: no filtering, no blending, and an 8-bit non-sRGB
//! target, so every pixel on screen has the bytes the still PNG has.

use crate::Player;
use clairlune_render::PixelRect;

const SHADER: &str = r#"
@group(0) @binding(0) var frame: texture_2d<u32>;
@group(0) @binding(1) var<uniform> palette: array<vec4<u32>, 256>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // One triangle covering the whole target.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let index = textureLoad(frame, vec2<i32>(p.xy), 0).r;
    let c = palette[index];
    return vec4<f32>(vec3<f32>(c.rgb) / 255.0, 1.0);
}
"#;

/// The device and the one pipeline per target format.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

/// One monitor's frame on the GPU.
pub struct Canvas {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    width: u32,
    height: u32,
    /// Two upload buffers used in turn, written in place while mapped: a
    /// frame's changed rectangles are packed straight into one, with no
    /// copy in between and no buffer made for every frame (each grown as
    /// needed).
    staging: [Option<wgpu::Buffer>; 2],
    next: usize,
}

/// Joins rectangles that sit side by side on the same rows (particles in
/// one cell row), so a frame needs fewer copies.
pub fn merge_spans(rects: &[PixelRect]) -> Vec<PixelRect> {
    let mut sorted = rects.to_vec();
    sorted.sort_unstable_by_key(|r| (r.y, r.h, r.x));
    let mut out: Vec<PixelRect> = Vec::with_capacity(sorted.len());
    for r in sorted {
        if let Some(last) = out.last_mut()
            && last.y == r.y
            && last.h == r.h
            && r.x <= last.x + last.w
        {
            let right = (last.x + last.w).max(r.x + r.w);
            last.w = right - last.x;
            continue;
        }
        out.push(r);
    }
    out
}

/// An 8-bit format whose stored bytes equal the shader's output bytes.
pub fn exact_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};
    formats
        .iter()
        .copied()
        .find(|f| matches!(f, Bgra8Unorm | Rgba8Unorm))
}

impl Gpu {
    /// Opens a device able to present to `compatible` (or any device, for
    /// drawing off-screen): the adapter with PCI ids `prefer` when there is
    /// one (the adapter the monitors are plugged into), else a low-power one.
    pub fn new(
        instance: &wgpu::Instance,
        compatible: Option<&wgpu::Surface<'_>>,
        prefer: Option<(u32, u32)>,
        allow_software: bool,
    ) -> Result<Gpu, String> {
        let preferred = prefer.and_then(|(vendor, device)| {
            pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .find(|a| {
                    let info = a.get_info();
                    info.vendor == vendor
                        && info.device == device
                        && compatible.is_none_or(|s| a.is_surface_supported(s))
                })
        });
        let found = match preferred {
            Some(a) => Ok(a),
            None => pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: compatible,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })),
        };
        let adapter = found
            .or_else(|e| {
                if !allow_software {
                    return Err(e);
                }
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: compatible,
                    force_fallback_adapter: true,
                    apply_limit_buckets: false,
                }))
            })
            .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("clairlune-live"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            // Small allocations: a wallpaper holds a few textures for hours,
            // and the default hints reserve large heaps up front.
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("live frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("live frame"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("live frame"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        Ok(Gpu {
            device,
            queue,
            adapter,
            layout,
            pipeline_layout,
            shader,
            pipelines: Vec::new(),
        })
    }

    fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return &self.pipelines[i].1;
        }
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("live frame"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        self.pipelines.push((format, pipeline));
        &self.pipelines.last().expect("just pushed").1
    }

    /// Puts the player's current frame and palette on the GPU.
    pub fn canvas(&self, player: &Player) -> Canvas {
        let (width, height) = player.size();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("live frame indices"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut palette = Vec::with_capacity(256 * 16);
        for c in player.colors() {
            for v in [(c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF, 255] {
                palette.extend_from_slice(&v.to_le_bytes());
            }
        }
        let palette_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("live palette"),
            size: palette.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&palette_buf, 0, &palette);
        let view = texture.create_view(&Default::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("live frame"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: palette_buf.as_entire_binding(),
                },
            ],
        });
        let mut canvas = Canvas {
            texture,
            bind,
            width,
            height,
            staging: [None, None],
            next: 0,
        };
        self.upload(
            &mut canvas,
            player,
            &[PixelRect {
                x: 0,
                y: 0,
                w: width,
                h: height,
            }],
        );
        canvas
    }

    /// Copies those rectangles of the player's frame to the canvas: one
    /// upload buffer and one command buffer, however many rectangles.
    pub fn upload(&self, canvas: &mut Canvas, player: &Player, rects: &[PixelRect]) {
        if rects.is_empty() {
            return;
        }
        debug_assert_eq!(player.size(), (canvas.width, canvas.height));
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let spans = merge_spans(rects);
        let mut copies = Vec::with_capacity(spans.len());
        let mut needed = 0u64;
        for r in &spans {
            let pitch = (r.w as usize).div_ceil(align) * align;
            copies.push((*r, needed, pitch as u32));
            needed += (pitch * r.h as usize) as u64;
        }
        let slot = canvas.next;
        canvas.next ^= 1;
        let fits = canvas.staging[slot]
            .as_ref()
            .is_some_and(|b| b.size() >= needed);
        if fits {
            // Used two frames ago, long done: mapped again at once.
            let buffer = canvas.staging[slot].as_ref().expect("fits");
            buffer.map_async(wgpu::MapMode::Write, .., |r| {
                if let Err(e) = r {
                    tracing::warn!(target: "clairlune", error = %e, "live upload buffer didn't map");
                }
            });
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        } else {
            canvas.staging[slot] = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("live upload"),
                size: needed.next_power_of_two().max(wgpu::COPY_BUFFER_ALIGNMENT),
                usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: true,
            }));
        }
        let staging = canvas.staging[slot].as_ref().expect("just made");
        {
            let Ok(mut view) = staging.get_mapped_range_mut(..) else {
                return;
            };
            for (r, offset, pitch) in &copies {
                for (i, y) in (r.y..r.y + r.h).enumerate() {
                    let at = *offset as usize + i * *pitch as usize;
                    view.slice(at..at + r.w as usize)
                        .copy_from_slice(player.index_row(r.x, y, r.w));
                }
            }
        }
        staging.unmap();
        let mut enc = self.device.create_command_encoder(&Default::default());
        for (r, offset, pitch) in copies {
            enc.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(pitch),
                        rows_per_image: None,
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &canvas.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: r.x,
                        y: r.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: r.w,
                    height: r.h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.queue.submit([enc.finish()]);
    }

    /// Draws the canvas into `target` (a surface texture or an off-screen
    /// one) and submits.
    pub fn draw(
        &mut self,
        canvas: &Canvas,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) {
        let pipeline = self.pipeline(format).clone();
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("live frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &canvas.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
    }

    /// Draws the canvas off-screen and reads it back as RGBA (tests).
    pub fn read_back(&mut self, canvas: &Canvas) -> Vec<u8> {
        let (w, h) = (canvas.width, canvas.height);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("read back"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.draw(canvas, &target.create_view(&Default::default()), format);
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read back"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        buffer.map_async(wgpu::MapMode::Read, .., |r| r.expect("map"));
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buffer.get_mapped_range(..).expect("mapped");
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let start = y * row as usize;
            out.extend_from_slice(&data[start..start + (w * 4) as usize]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_join_neighbours_on_the_same_rows_only() {
        let r = |x, y, w, h| PixelRect { x, y, w, h };
        let spans = merge_spans(&[
            r(32, 0, 16, 32),
            r(0, 0, 16, 32),
            r(16, 0, 16, 32),
            r(80, 0, 16, 32),
            r(16, 32, 16, 32),
        ]);
        assert_eq!(
            spans,
            [r(0, 0, 48, 32), r(80, 0, 16, 32), r(16, 32, 16, 32)]
        );
        // Overlapping boxes (a sprite over a particle) join too.
        assert_eq!(
            merge_spans(&[r(0, 0, 20, 8), r(10, 0, 30, 8)]),
            [r(0, 0, 40, 8)]
        );
    }
    use clairlune_render::ScenePackage;
    use clairlune_render::clock::Clock;
    use clairlune_render::to_rgba;
    use clairlune_scene::{Aspect, ContentContext, compose};
    use std::sync::Arc;

    /// A device to draw with, or `None` where there is none (CI without a
    /// GPU or software rasteriser): the test then says so and passes.
    fn gpu() -> Option<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        match Gpu::new(&instance, None, None, true) {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("skipped: {e}");
                None
            }
        }
    }

    #[test]
    fn gpu_frames_have_the_still_bytes_and_partial_uploads_keep_them() {
        let Some(mut gpu) = gpu() else {
            return;
        };
        let r = compose(7, Aspect::from_size(320, 180), &ContentContext::default());
        let pkg = Arc::new(ScenePackage::capture(&r, 320, 180).unwrap());
        let theme = pkg.themes()[0];
        let mut player = Player::new(pkg.clone(), theme, true).unwrap();
        let t0 = pkg.hero_ms(theme).unwrap();
        player.advance(t0);
        let mut canvas = gpu.canvas(&player);
        let (still, palette) = pkg.still(theme, true).unwrap();
        assert_eq!(
            gpu.read_back(&canvas),
            to_rgba(&still, palette),
            "hero frame"
        );
        for k in 1..12u64 {
            let t = t0 + k * 83;
            let rects = player.advance(t);
            gpu.upload(&mut canvas, &player, &rects);
            let (full, palette) = pkg.frame(theme, true, t, Clock::Live).unwrap();
            assert_eq!(gpu.read_back(&canvas), to_rgba(&full, palette), "t {t}");
        }
        eprintln!("drew on {}", gpu.adapter.get_info().name);
    }
}
