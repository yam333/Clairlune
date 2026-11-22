//! Showing a player's frame on macOS through two IOSurfaces: the live
//! wallpaper's windows draw this way.
//!
//! A view hosts a plain Core Animation layer whose contents is one of two
//! IOSurfaces (BGRA, the view's size in pixels). Each tick writes only the
//! rectangles that changed into the surface not on screen (plus what it
//! missed while the other was shown), then shows it. The window server
//! composites the surface as it is: no GPU device or swapchain in the app,
//! no drawing or display lists in AppKit, and the CPU touches only the
//! pixels that changed.

use crate::Player;
use clairlune_render::PixelRect;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSView;
use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_foundation::NSRect;
use objc2_io_surface::{
    IOSurfaceLockOptions, IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceColorSpace,
    kIOSurfaceHeight, kIOSurfacePixelFormat, kIOSurfaceWidth,
};
use objc2_quartz_core::{CALayer, CATransaction, kCAFilterNearest};

/// 'BGRA': one little-endian 0xAARRGGBB word per pixel.
const BGRA: i32 = 0x4247_5241;

/// Past this many rectangles a surface misses, it takes their bounding
/// box instead (one copy beats many tiny ones).
const MAX_MISSING: usize = 48;

struct Buffer {
    surface: CFRetained<IOSurfaceRef>,
    /// What changed since this surface was last written.
    missing: Vec<PixelRect>,
}

/// A view showing a player's frame.
pub struct SurfaceView {
    view: Retained<NSView>,
    layer: Retained<CALayer>,
    player: Player,
    buffers: [Buffer; 2],
    /// The buffer on screen.
    shown: Option<usize>,
}

fn new_surface(w: u32, h: u32) -> Option<CFRetained<IOSurfaceRef>> {
    let num = |v: u32| CFNumber::new_i32(v as i32);
    let (width, height, bpe, format) = (
        num(w),
        num(h),
        CFNumber::new_i32(4),
        CFNumber::new_i32(BGRA),
    );
    // SAFETY: the keys are IOSurface's own constants.
    let props = unsafe {
        CFDictionary::<CFString, CFNumber>::from_slices(
            &[
                kIOSurfaceWidth,
                kIOSurfaceHeight,
                kIOSurfaceBytesPerElement,
                kIOSurfacePixelFormat,
            ],
            &[&width, &height, &bpe, &format],
        )
    };
    // SAFETY: a dictionary of IOSurface properties.
    let surface = unsafe { IOSurfaceRef::new(props.as_opaque()) }?;
    // Its pixels are sRGB, like the still: tagged, so the screen shows them
    // the same.
    // SAFETY: a constant colour space name; the value is its ICC profile.
    unsafe {
        if let Some(icc) = CGColorSpace::with_name(Some(kCGColorSpaceSRGB))
            .and_then(|s| CGColorSpace::icc_data(Some(&s)))
        {
            surface.set_value(kIOSurfaceColorSpace, &icc);
        }
    }
    Some(surface)
}

impl SurfaceView {
    /// A view of `frame` (points) showing `player` (its size is the view's
    /// in pixels), at its current frame.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, player: Player) -> Option<SurfaceView> {
        let (w, h) = player.size();
        let whole = PixelRect { x: 0, y: 0, w, h };
        let buffer = || {
            Some(Buffer {
                surface: new_surface(w, h)?,
                missing: vec![whole],
            })
        };
        let buffers = [buffer()?, buffer()?];
        let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
        let layer = CALayer::new();
        layer.setOpaque(true);
        // Pixel for pixel; should the view's size differ, no blur.
        // SAFETY: a constant filter name.
        unsafe {
            layer.setMagnificationFilter(kCAFilterNearest);
            layer.setMinificationFilter(kCAFilterNearest);
        }
        // A layer of its own (layer-hosting): AppKit draws nothing into it.
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        let mut out = SurfaceView {
            view,
            layer,
            player,
            buffers,
            shown: None,
        };
        out.present();
        Some(out)
    }

    /// The view, to put in a window or another view.
    pub fn view(&self) -> &NSView {
        &self.view
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    /// Moves the player to now and shows what changed; returns the pixels
    /// and rectangles that changed.
    pub fn advance(&mut self) -> (u64, usize) {
        let t = self.player.now_ms();
        let rects = self.player.advance(t);
        if rects.is_empty() {
            return (0, 0);
        }
        let pixels = rects.iter().map(|r| u64::from(r.w * r.h)).sum();
        for b in &mut self.buffers {
            b.missing.extend_from_slice(&rects);
            if b.missing.len() > MAX_MISSING {
                let all = bounding(&b.missing);
                b.missing = vec![all];
            }
        }
        self.present();
        (pixels, rects.len())
    }

    /// Writes what the surface off screen misses and shows it. When the
    /// window server still reads that surface, it waits for the next tick.
    fn present(&mut self) {
        let next = match self.shown {
            Some(i) => 1 - i,
            None => 0,
        };
        let b = &mut self.buffers[next];
        if b.missing.is_empty() || (self.shown.is_some() && b.surface.is_in_use()) {
            return;
        }
        // SAFETY: locked for writing; the base address and row stride are
        // the surface's, valid until unlock, and every write lands inside
        // its w x h pixels (the rects are inside the player's frame, which
        // is the surface's size).
        unsafe {
            if b.surface
                .lock(IOSurfaceLockOptions::empty(), std::ptr::null_mut())
                != 0
            {
                return;
            }
            let base = b.surface.base_address().as_ptr().cast::<u8>();
            let stride = b.surface.bytes_per_row();
            let colors = self.player.colors();
            for r in &b.missing {
                for y in r.y..r.y + r.h {
                    let row = base
                        .add(y as usize * stride)
                        .cast::<u32>()
                        .add(r.x as usize);
                    let out = std::slice::from_raw_parts_mut(row, r.w as usize);
                    for (o, &i) in out.iter_mut().zip(self.player.index_row(r.x, y, r.w)) {
                        *o = 0xFF00_0000 | colors[usize::from(i)];
                    }
                }
            }
            b.surface
                .unlock(IOSurfaceLockOptions::empty(), std::ptr::null_mut());
        }
        b.missing.clear();
        CATransaction::begin();
        // No cross-fade: contents swap at once.
        CATransaction::setDisableActions(true);
        // SAFETY: an IOSurfaceRef is an object (toll-free bridged with
        // IOSurface), which a layer takes as its contents.
        unsafe {
            let obj = &*(&*b.surface as *const IOSurfaceRef).cast::<AnyObject>();
            self.layer.setContents(Some(obj));
        }
        CATransaction::commit();
        self.shown = Some(next);
    }
}

/// The smallest rectangle holding them all.
fn bounding(rects: &[PixelRect]) -> PixelRect {
    let x0 = rects.iter().map(|r| r.x).min().unwrap_or(0);
    let y0 = rects.iter().map(|r| r.y).min().unwrap_or(0);
    let x1 = rects.iter().map(|r| r.x + r.w).max().unwrap_or(0);
    let y1 = rects.iter().map(|r| r.y + r.h).max().unwrap_or(0);
    PixelRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}
