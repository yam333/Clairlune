//! Drawing a player's frame into an `NSView` with Core Graphics (macOS):
//! the live wallpaper's windows and the screen saver both draw this way.
//!
//! The frame stays palette indices; each changed rectangle becomes a small
//! indexed `CGImage` (the palette as its colour table, in sRGB like the
//! still), drawn pixel for pixel in the view's `drawRect:`. AppKit keeps
//! the rest of the view as it was, so a tick costs only what changed, and
//! there is no GPU device, swapchain or full-screen texture to keep: the
//! view's own backing store is the only copy of the screen.

use crate::Player;
use clairlune_render::PixelRect;
use objc2::rc::{Allocated, Retained};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSGraphicsContext, NSResponder, NSView};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGContext, CGDataProvider, CGImage,
    CGImageAlphaInfo, CGInterpolationQuality, kCGColorSpaceSRGB,
};
use objc2_foundation::{NSInteger, NSObject, NSPoint, NSRect, NSSize};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

/// Draws one player's frame: its palette as a Core Graphics colour space,
/// made once.
pub struct Painter {
    space: CFRetained<CGColorSpace>,
}

impl Painter {
    pub fn new(player: &Player) -> Option<Painter> {
        let mut table = [[0u8; 3]; 256];
        for (rgb, c) in table.iter_mut().zip(player.colors()) {
            *rgb = [(c >> 16) as u8, (c >> 8) as u8, *c as u8];
        }
        // SAFETY: CGColorSpaceCreateWithName is called with a constant
        // name; CGColorSpaceCreateIndexed copies the 256-entry table.
        let space = unsafe {
            let base = CGColorSpace::with_name(Some(kCGColorSpaceSRGB))
                .or_else(CGColorSpace::new_device_rgb)?;
            CGColorSpace::new_indexed(Some(&base), 255, table.as_ptr().cast())?
        };
        Some(Painter { space })
    }

    /// Draws what `view` is asked to draw now (call from `drawRect:`): each
    /// rectangle AppKit wants, from the player's frame. The player's size
    /// is the view's in pixels.
    pub fn draw(&self, view: &NSView, player: &Player) {
        let Some(ctx) = NSGraphicsContext::currentContext() else {
            return;
        };
        let cg = ctx.CGContext();
        let map = Map::new(view, player);
        let mut list: *const NSRect = std::ptr::null();
        let mut count: NSInteger = 0;
        // SAFETY: both pointers are valid; AppKit points `list` at its own
        // array of `count` rects, valid during drawRect:.
        unsafe { view.getRectsBeingDrawn_count(&mut list, &mut count) };
        let rects: Vec<NSRect> = if list.is_null() || count <= 0 {
            vec![view.bounds()]
        } else {
            // SAFETY: as above.
            unsafe { std::slice::from_raw_parts(list, count as usize) }.to_vec()
        };
        CGContext::save_g_state(Some(&cg));
        CGContext::set_interpolation_quality(Some(&cg), CGInterpolationQuality::None);
        for r in rects {
            let Some(px) = map.to_pixels(r) else {
                continue;
            };
            if let Some(image) = self.image(player, &px) {
                CGContext::draw_image(Some(&cg), map.to_points(&px), Some(&image));
            }
        }
        CGContext::restore_g_state(Some(&cg));
    }

    /// The pixels of `r` (view coordinates) as an indexed image of their
    /// own (a copy: Core Graphics may keep the image after the draw).
    fn image(&self, player: &Player, r: &PixelRect) -> Option<CFRetained<CGImage>> {
        let (w, h) = (r.w as usize, r.h as usize);
        let mut bytes = Vec::with_capacity(w * h);
        for y in r.y..r.y + r.h {
            bytes.extend_from_slice(player.index_row(r.x, y, r.w));
        }
        let bytes = Box::new(bytes);
        let (data, size) = (bytes.as_ptr().cast::<c_void>(), bytes.len());
        let info = Box::into_raw(bytes).cast::<c_void>();

        unsafe extern "C-unwind" fn release(info: *mut c_void, _data: NonNull<c_void>, _: usize) {
            // SAFETY: `info` is the Box made above, given back once.
            drop(unsafe { Box::from_raw(info.cast::<Vec<u8>>()) });
        }
        // SAFETY: the provider owns `info` (the bytes) and frees it through
        // `release` when the last image using it goes; `data` points into
        // it, `size` bytes.
        let provider = unsafe { CGDataProvider::with_data(info, data, size, Some(release)) };
        let Some(provider) = provider else {
            // Never handed over: free it here.
            // SAFETY: `info` is the Box made above, not given to anyone.
            drop(unsafe { Box::from_raw(info.cast::<Vec<u8>>()) });
            return None;
        };
        // SAFETY: an 8-bit indexed image, one byte per pixel, rows `w`
        // bytes apart, over a provider holding w * h bytes.
        unsafe {
            CGImage::new(
                w,
                h,
                8,
                8,
                w,
                Some(&self.space),
                CGBitmapInfo(CGImageAlphaInfo::None.0),
                Some(&provider),
                std::ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
    }
}

/// Asks AppKit to redraw these rectangles (view coordinates in pixels, as
/// [`Player::advance`] gives them) of `view`.
pub fn invalidate(view: &NSView, player: &Player, rects: &[PixelRect]) {
    let map = Map::new(view, player);
    for r in rects {
        view.setNeedsDisplayInRect(map.to_points(r));
    }
}

/// Between the view's points (origin at the bottom left) and the player's
/// pixels (origin at the top left).
struct Map {
    origin: NSPoint,
    /// Pixels per point.
    sx: f64,
    sy: f64,
    w: u32,
    h: u32,
}

impl Map {
    fn new(view: &NSView, player: &Player) -> Map {
        let b = view.bounds();
        let (w, h) = player.size();
        Map {
            origin: b.origin,
            sx: f64::from(w) / b.size.width.max(1.0),
            sy: f64::from(h) / b.size.height.max(1.0),
            w,
            h,
        }
    }

    /// The pixels covering a rectangle of points, inside the frame.
    fn to_pixels(&self, r: NSRect) -> Option<PixelRect> {
        let clamp = |v: f64, max: u32| v.clamp(0.0, f64::from(max)) as u32;
        let left = (r.origin.x - self.origin.x) * self.sx;
        let right = (r.origin.x + r.size.width - self.origin.x) * self.sx;
        let bottom = (r.origin.y - self.origin.y) * self.sy;
        let top = (r.origin.y + r.size.height - self.origin.y) * self.sy;
        let x0 = clamp(left.floor(), self.w);
        let x1 = clamp(right.ceil(), self.w);
        let y0 = clamp((f64::from(self.h) - top).floor(), self.h);
        let y1 = clamp((f64::from(self.h) - bottom).ceil(), self.h);
        (x1 > x0 && y1 > y0).then(|| PixelRect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }

    /// Where those pixels are, in points.
    fn to_points(&self, r: &PixelRect) -> NSRect {
        NSRect::new(
            NSPoint::new(
                self.origin.x + f64::from(r.x) / self.sx,
                self.origin.y + f64::from(self.h - r.y - r.h) / self.sy,
            ),
            NSSize::new(f64::from(r.w) / self.sx, f64::from(r.h) / self.sy),
        )
    }
}

/// A view showing a player's frame (the live wallpaper's windows).
pub struct FrameIvars {
    playing: RefCell<Option<(Player, Painter)>>,
    /// Time spent in drawRect: since it was last asked.
    drawing: Cell<Duration>,
}

define_class!(
    // SAFETY: NSView asks nothing of subclasses that only draw; there is
    // no Drop.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ClairluneFrameView"]
    #[ivars = FrameIvars]
    pub struct FrameView;

    impl FrameView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let started = Instant::now();
            if let Some((player, painter)) = self.ivars().playing.borrow().as_ref() {
                painter.draw(self, player);
            }
            let d = self.ivars().drawing.get();
            self.ivars().drawing.set(d + started.elapsed());
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            true
        }
    }
);

impl FrameView {
    /// A view of `frame` (points) showing `player`, which has moved to its
    /// first frame already.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, player: Player) -> Option<Retained<Self>> {
        let painter = Painter::new(&player)?;
        let this: Allocated<Self> = Self::alloc(mtm);
        let this = this.set_ivars(FrameIvars {
            playing: RefCell::new(Some((player, painter))),
            drawing: Cell::new(Duration::ZERO),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        Some(view)
    }

    /// Moves the player to now and asks for what changed to be drawn;
    /// returns the pixels and rectangles that changed.
    pub fn advance(&self) -> (u64, usize) {
        // AppKit draws later (drawRect: borrows again then), not inside
        // setNeedsDisplayInRect:.
        let mut playing = self.ivars().playing.borrow_mut();
        let Some((player, _)) = playing.as_mut() else {
            return (0, 0);
        };
        let t = player.now_ms();
        let rects = player.advance(t);
        let pixels = rects.iter().map(|r| u64::from(r.w * r.h)).sum();
        invalidate(self, player, &rects);
        (pixels, rects.len())
    }

    /// Time spent drawing since the last call.
    pub fn take_drawing_time(&self) -> Duration {
        self.ivars().drawing.replace(Duration::ZERO)
    }

    /// Whether the scene is a journey.
    pub fn is_journey(&self) -> bool {
        self.ivars()
            .playing
            .borrow()
            .as_ref()
            .is_some_and(|(p, _)| p.is_journey())
    }
}
