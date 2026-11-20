//! The light presenter on Windows: Direct3D 11 with a flip-sequential
//! swapchain, which keeps each buffer's pixels between frames.
//!
//! A frame copies only the rectangles that changed into the back buffer (no
//! shader, no full-screen draw) and presents with those rectangles as dirty
//! rects, so the compositor recomposes only them. With two buffers, the back
//! buffer holds the frame before last, so it gets this frame's and the last
//! frame's rectangles.

use crate::Player;
use clairlune_render::PixelRect;
use std::ffi::c_void;
use windows::Win32::Foundation::{HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_PRESENT, DXGI_PRESENT_PARAMETERS, DXGI_SCALING_NONE,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
    IDXGIAdapter1, IDXGIFactory2, IDXGISwapChain1,
};

/// One Direct3D 11 device for every monitor.
pub struct D3d11 {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    factory: IDXGIFactory2,
    /// The adapter's name, for logs.
    pub name: String,
}

/// One window's swapchain.
pub struct FlipSurface {
    swapchain: IDXGISwapChain1,
    width: u32,
    height: u32,
    /// The last frame's changed rectangles (what the other buffer lacks).
    last: Vec<PixelRect>,
    /// Frames presented, capped: the first two are written whole.
    presented: u8,
    /// Reused per frame: one rectangle's pixels.
    scratch: Vec<u32>,
}

fn err(what: &str, e: windows::core::Error) -> String {
    format!("{what}: {e}")
}

impl D3d11 {
    /// A device on the adapter with these PCI ids, else the first hardware
    /// adapter.
    pub fn new(prefer: Option<(u32, u32)>) -> Result<D3d11, String> {
        let factory: IDXGIFactory2 =
            unsafe { CreateDXGIFactory1() }.map_err(|e| err("DXGI factory", e))?;
        let mut chosen: Option<(IDXGIAdapter1, String)> = None;
        for i in 0.. {
            let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else {
                break;
            };
            let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
                continue;
            };
            // Skip the software rasteriser (flag 2: DXGI_ADAPTER_FLAG_SOFTWARE).
            if desc.Flags & 2 != 0 {
                continue;
            }
            let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(128);
            let name = String::from_utf16_lossy(&desc.Description[..len]);
            let wanted = prefer.is_some_and(|(v, d)| desc.VendorId == v && desc.DeviceId == d);
            if wanted || chosen.is_none() {
                chosen = Some((adapter, name));
                if wanted {
                    break;
                }
            }
        }
        let (adapter, name) = chosen.ok_or("no graphics adapter")?;
        let (mut device, mut context) = (None, None);
        unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map_err(|e| err("Direct3D 11 device", e))?;
        Ok(D3d11 {
            device: device.ok_or("no device")?,
            context: context.ok_or("no device context")?,
            factory,
            name,
        })
    }

    /// A swapchain for a window of this size.
    pub fn surface(&self, hwnd: HWND, width: u32, height: u32) -> Result<FlipSurface, String> {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_NONE,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            ..Default::default()
        };
        let swapchain = unsafe {
            self.factory
                .CreateSwapChainForHwnd(&self.device, hwnd, &desc, None, None)
        }
        .map_err(|e| err("swapchain", e))?;
        Ok(FlipSurface {
            swapchain,
            width,
            height,
            last: Vec::new(),
            presented: 0,
            scratch: Vec::new(),
        })
    }

    /// Copies those rectangles of the player's frame into the surface's
    /// back buffer.
    fn write(
        &self,
        s: &mut FlipSurface,
        player: &Player,
        rects: &[PixelRect],
    ) -> Result<(), String> {
        let back: ID3D11Texture2D =
            unsafe { s.swapchain.GetBuffer(0) }.map_err(|e| err("back buffer", e))?;
        for r in rects {
            player.rect_pixels(r, &mut s.scratch);
            let bx = D3D11_BOX {
                left: r.x,
                top: r.y,
                front: 0,
                right: r.x + r.w,
                bottom: r.y + r.h,
                back: 1,
            };
            unsafe {
                self.context.UpdateSubresource(
                    &back,
                    0,
                    Some(&bx),
                    s.scratch.as_ptr() as *const c_void,
                    r.w * 4,
                    0,
                )
            };
        }
        Ok(())
    }

    /// Shows the player's current frame: `changed` are the rectangles that
    /// changed since the last frame; `whole` redraws everything. Returns
    /// the pixels copied (0 when nothing changed and nothing was shown).
    pub fn present(
        &self,
        s: &mut FlipSurface,
        player: &Player,
        changed: &[PixelRect],
        whole: bool,
    ) -> Result<u64, String> {
        match self.stage(s, player, changed, whole)? {
            Some((pixels, dirty)) => {
                self.flip(s, dirty)?;
                Ok(pixels)
            }
            None => Ok(0),
        }
    }

    /// Brings the back buffer up to the player's frame; returns the pixels
    /// copied and the rectangles to present as dirty (`None`: nothing to
    /// show; an empty list: the whole buffer).
    fn stage(
        &self,
        s: &mut FlipSurface,
        player: &Player,
        changed: &[PixelRect],
        whole: bool,
    ) -> Result<Option<(u64, Vec<PixelRect>)>, String> {
        let changed = crate::gpu::merge_spans(changed);
        let first = whole || s.presented < 2;
        if !first && changed.is_empty() && s.last.is_empty() {
            return Ok(None);
        }
        let to_write: Vec<PixelRect> = if first {
            vec![PixelRect {
                x: 0,
                y: 0,
                w: s.width,
                h: s.height,
            }]
        } else {
            // The back buffer holds the frame before last.
            let mut both: Vec<PixelRect> = s.last.iter().chain(&changed).copied().collect();
            both.sort_unstable();
            both.dedup();
            both
        };
        self.write(s, player, &to_write)?;
        let pixels = to_write.iter().map(|r| u64::from(r.w * r.h)).sum();
        s.last = changed.clone();
        Ok(Some((pixels, if first { Vec::new() } else { changed })))
    }

    /// Presents the back buffer, telling the compositor what changed.
    fn flip(&self, s: &mut FlipSurface, dirty: Vec<PixelRect>) -> Result<(), String> {
        let mut rects: Vec<RECT> = dirty
            .iter()
            .map(|r| RECT {
                left: r.x as i32,
                top: r.y as i32,
                right: (r.x + r.w) as i32,
                bottom: (r.y + r.h) as i32,
            })
            .collect();
        let params = DXGI_PRESENT_PARAMETERS {
            DirtyRectsCount: rects.len() as u32,
            pDirtyRects: if rects.is_empty() {
                std::ptr::null_mut()
            } else {
                rects.as_mut_ptr()
            },
            pScrollRect: std::ptr::null_mut(),
            pScrollOffset: std::ptr::null_mut(),
        };
        unsafe { s.swapchain.Present1(0, DXGI_PRESENT(0), &params) }
            .ok()
            .map_err(|e| err("present", e))?;
        s.presented = s.presented.saturating_add(1).min(2);
        Ok(())
    }

    /// Fills the window with one 0RGB colour and presents it (probes).
    #[doc(hidden)]
    pub fn fill(&self, s: &mut FlipSurface, rgb: u32) -> Result<(), String> {
        let back: ID3D11Texture2D =
            unsafe { s.swapchain.GetBuffer(0) }.map_err(|e| err("back buffer", e))?;
        let pixels = vec![rgb; (s.width * s.height) as usize];
        unsafe {
            self.context.UpdateSubresource(
                &back,
                0,
                None,
                pixels.as_ptr() as *const c_void,
                s.width * 4,
                0,
            )
        };
        self.flip(s, Vec::new())
    }

    /// The back buffer's pixels as RGBA (tests).
    pub fn read_back(&self, s: &FlipSurface) -> Result<Vec<u8>, String> {
        let back: ID3D11Texture2D =
            unsafe { s.swapchain.GetBuffer(0) }.map_err(|e| err("back buffer", e))?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { back.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut staging)) }
            .map_err(|e| err("staging texture", e))?;
        let staging = staging.ok_or("no staging texture")?;
        unsafe { self.context.CopyResource(&staging, &back) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(|e| err("map", e))?;
        let mut out = Vec::with_capacity((s.width * s.height * 4) as usize);
        for y in 0..s.height as usize {
            let row = unsafe {
                std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                    s.width as usize * 4,
                )
            };
            for px in row.as_chunks::<4>().0 {
                out.extend_from_slice(&[px[2], px[1], px[0], 255]);
            }
        }
        unsafe { self.context.Unmap(&staging, 0) };
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_render::ScenePackage;
    use clairlune_render::clock::Clock;
    use clairlune_render::to_rgba;
    use clairlune_scene::{Aspect, ContentContext, compose};
    use std::sync::Arc;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
    };
    use windows::core::w;

    /// The swapchain's back buffer holds exactly the CPU frame after every
    /// present, though each frame copies only what changed. Uses a hidden
    /// window, so nothing shows; skips where there's no Direct3D 11 device.
    #[test]
    fn flip_frames_equal_the_cpu_frames() {
        let Ok(gpu) = D3d11::new(None) else {
            eprintln!("skipped: no Direct3D 11 device");
            return;
        };
        let r = compose(7, Aspect::from_size(320, 180), &ContentContext::default());
        let pkg = Arc::new(ScenePackage::capture(&r, 320, 180).unwrap());
        let theme = pkg.themes()[0];
        let mut player = Player::new(pkg.clone(), theme, true).unwrap();
        let module = unsafe { GetModuleHandleW(None) }.unwrap();
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("clairlune flip test"),
                WS_POPUP,
                0,
                0,
                320,
                180,
                None,
                None,
                Some(module.into()),
                None,
            )
        }
        .unwrap();
        let mut surface = gpu.surface(hwnd, 320, 180).unwrap();
        let t0 = pkg.hero_ms(theme).unwrap();
        let mut copied = Vec::new();
        for k in 0..16u64 {
            let t = t0 + k * 83;
            let rects = player.advance(t);
            let staged = gpu.stage(&mut surface, &player, &rects, false).unwrap();
            let (full, palette) = pkg.frame(theme, true, t, Clock::Live).unwrap();
            assert_eq!(
                gpu.read_back(&surface).unwrap(),
                to_rgba(&full, palette),
                "frame {k}"
            );
            if let Some((pixels, dirty)) = staged {
                copied.push(pixels);
                gpu.flip(&mut surface, dirty).unwrap();
            }
        }
        // After the first two frames, only small parts are copied.
        let whole = 320 * 180;
        assert!(copied[..2].iter().all(|&p| p == whole));
        assert!(copied[2..].iter().all(|&p| p < whole / 2), "{copied:?}");
        eprintln!("drew on {}", gpu.name);
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}
