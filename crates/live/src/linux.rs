//! The live wallpaper on Linux, where a surface of our own can sit under
//! everything without hiding the desktop's icons:
//!
//! - Wayland compositors with the wlr layer shell (Sway, Hyprland, river,
//!   Wayfire, labwc...): a background-layer surface per output, drawn in
//!   shared memory ([`clairlune_platform::wlr::Surfaces`]).
//! - X11 window managers without a desktop of their own (i3, Openbox,
//!   bspwm...): a window per monitor at the bottom of the stack, ignoring
//!   the mouse, drawn with `PutImage`.
//!
//! Desktops that draw their own desktop and icons (GNOME, KDE Plasma, XFCE,
//! Cinnamon, MATE, LXQt) would be covered by such a surface, so there the
//! still frame stays the wallpaper.
//!
//! Linux composes one scene for the whole desktop, at the largest output's
//! size; each output shows the largest centred part of it with its shape.
//! Only the rectangles that changed are copied. Untried: written and
//! compile-checked without a Linux desktop.

use crate::{CoverPolicy, FrameStats, Player};
use clairlune_platform::Monitor;
use clairlune_platform::wlr::{Rect, Surfaces};
use clairlune_render::PixelRect;
use std::time::{Duration, Instant};
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::Event;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    self, ConfigureWindowAux, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask,
    ImageFormat, StackMode, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// Frame rate cap: every changed pixel goes through the CPU.
const MAX_FPS: u32 = 30;

/// Desktops that draw their own desktop and icons.
const OWN_DESKTOPS: [&str; 10] = [
    "gnome",
    "kde",
    "xfce",
    "x-cinnamon",
    "cinnamon",
    "mate",
    "lxqt",
    "lxde",
    "unity",
    "budgie",
];

/// A desktop from `XDG_CURRENT_DESKTOP` that draws its own desktop, if any.
pub fn own_desktop(current: &str) -> Option<String> {
    current
        .split(':')
        .map(str::to_ascii_lowercase)
        .find(|p| OWN_DESKTOPS.contains(&p.as_str()))
}

/// Whether live wallpapers can play in this session, with the reason if
/// not.
pub fn available() -> Result<(), String> {
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if let Some(d) = own_desktop(&current) {
        return Err(format!(
            "{d} draws its own desktop and icons, which a live wallpaper would cover"
        ));
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        clairlune_platform::wlr::available()
    } else if std::env::var_os("DISPLAY").is_some() {
        let (conn, screen) = x11rb::connect(None).map_err(|e| format!("X11: {e}"))?;
        let root = conn.setup().roots[screen].root;
        match desktop_window(&conn, root)? {
            Some(name) => Err(format!(
                "the desktop ({name}) draws its own icons, which a live wallpaper would cover"
            )),
            None => Ok(()),
        }
    } else {
        Err("no display".into())
    }
}

/// The largest centred part of a `scene`-sized frame with an `output`'s
/// shape, cut at whole pixels: what a screen of that size shows without
/// scaling (X11), and where in the screen it goes when the scene is
/// smaller.
pub fn centred_view(scene: (u32, u32), output: (u32, u32)) -> (PixelRect, (i16, i16)) {
    let w = scene.0.min(output.0);
    let h = scene.1.min(output.1);
    let view = PixelRect {
        x: (scene.0 - w) / 2,
        y: (scene.1 - h) / 2,
        w,
        h,
    };
    let at = (((output.0 - w) / 2) as i16, ((output.1 - h) / 2) as i16);
    (view, at)
}

/// Live wallpapers on every screen, on Wayland or X11.
pub struct LinuxDesktop {
    host: Host,
    pub cover_policy: CoverPolicy,
}

enum Host {
    Wayland(Box<Wayland>),
    X11(Box<X11>),
}

impl LinuxDesktop {
    /// Plays the scene (Linux composes one, for the whole desktop) on every
    /// screen.
    pub fn start(scenes: Vec<(Monitor, Player)>) -> Result<LinuxDesktop, String> {
        available()?;
        let (monitor, player) = scenes.into_iter().next().ok_or("no scene to play")?;
        let host = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            Host::Wayland(Box::new(Wayland::start(monitor, player)?))
        } else {
            Host::X11(Box::new(X11::start(monitor, player)?))
        };
        Ok(LinuxDesktop {
            host,
            cover_policy: CoverPolicy::default(),
        })
    }

    /// "wayland (shm)" or "x11 (putimage)".
    pub fn presenter(&self) -> String {
        match &self.host {
            Host::Wayland(_) => "wayland (shm, layer shell)".into(),
            Host::X11(_) => "x11 (putimage)".into(),
        }
    }

    /// The highest frame rate worth asking for: the CPU copies every
    /// changed pixel.
    pub fn max_fps(&self) -> u32 {
        MAX_FPS
    }

    /// The scene's "monitor" (the whole desktop).
    pub fn monitors(&self) -> impl Iterator<Item = &Monitor> {
        std::iter::once(match &self.host {
            Host::Wayland(w) => &w.monitor,
            Host::X11(x) => &x.monitor,
        })
    }

    /// Draws the current time wherever it shows.
    pub fn tick(&mut self) -> Result<FrameStats, String> {
        match &mut self.host {
            Host::Wayland(w) => w.tick(),
            Host::X11(x) => x.tick(),
        }
    }
}

// ---- Wayland

struct Wayland {
    monitor: Monitor,
    player: Player,
    surfaces: Surfaces,
}

/// Writes rectangle `r` of the player's frame into an XRGB8888 canvas of
/// the frame's size.
fn fill(player: &Player, r: Rect, canvas: &mut [u8], stride: usize) {
    let colors = player.colors();
    for y in r.y..r.y + r.h {
        let row = player.index_row(r.x, y, r.w);
        let dst = &mut canvas[y as usize * stride + r.x as usize * 4..][..r.w as usize * 4];
        for (d, &i) in dst.as_chunks_mut::<4>().0.iter_mut().zip(row) {
            *d = colors[usize::from(i)].to_le_bytes();
        }
    }
}

impl Wayland {
    fn start(monitor: Monitor, mut player: Player) -> Result<Wayland, String> {
        let mut surfaces = Surfaces::new(player.size())?;
        let t = player.now_ms();
        player.advance(t);
        surfaces.present(&[], &mut |r, c, s| fill(&player, r, c, s))?;
        tracing::info!(
            target: "clairlune",
            outputs = ?surfaces.outputs(),
            "live: background surfaces on Wayland"
        );
        Ok(Wayland {
            monitor,
            player,
            surfaces,
        })
    }

    fn tick(&mut self) -> Result<FrameStats, String> {
        let started = Instant::now();
        self.surfaces.dispatch(Duration::ZERO)?;
        let t = self.player.now_ms();
        let rects = self.player.advance(t);
        let changed: Vec<Rect> = rects
            .iter()
            .map(|r| Rect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            })
            .collect();
        let t0 = Instant::now();
        let player = &self.player;
        let drawn = self
            .surfaces
            .present(&changed, &mut |r, c, s| fill(player, r, c, s))?;
        Ok(FrameStats {
            pixels: rects.iter().map(|r| u64::from(r.w * r.h)).sum::<u64>() * drawn as u64,
            rects: rects.len() * drawn,
            covered: 0,
            away: false,
            stalls: 0,
            busy: started.elapsed(),
            upload: t0.elapsed(),
            present: Duration::ZERO,
        })
    }
}

// ---- X11

struct XOutput {
    player: Player,
    window: xproto::Window,
    gc: xproto::Gcontext,
    /// Where the player's view goes in the window.
    at: (i16, i16),
    /// Exposed: draw everything again.
    exposed: bool,
}

struct X11 {
    monitor: Monitor,
    conn: RustConnection,
    depth: u8,
    outputs: Vec<XOutput>,
}

/// A window of type desktop (a desktop drawing its icons), by its class.
fn desktop_window(conn: &RustConnection, root: xproto::Window) -> Result<Option<String>, String> {
    let atom = |name: &[u8]| -> Result<xproto::Atom, String> {
        Ok(conn
            .intern_atom(false, name)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .atom)
    };
    let list = atom(b"_NET_CLIENT_LIST")?;
    let kind = atom(b"_NET_WM_WINDOW_TYPE")?;
    let desktop = atom(b"_NET_WM_WINDOW_TYPE_DESKTOP")?;
    let windows: Vec<u32> = conn
        .get_property(false, root, list, xproto::AtomEnum::WINDOW, 0, 4096)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    for w in windows {
        let Ok(reply) = conn
            .get_property(false, w, kind, xproto::AtomEnum::ATOM, 0, 32)
            .map_err(|e| e.to_string())?
            .reply()
        else {
            continue;
        };
        if reply.value32().is_some_and(|mut v| v.any(|a| a == desktop)) {
            let class = conn
                .get_property(
                    false,
                    w,
                    xproto::AtomEnum::WM_CLASS,
                    xproto::AtomEnum::STRING,
                    0,
                    64,
                )
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| {
                    String::from_utf8_lossy(&r.value)
                        .split('\0')
                        .find(|s| !s.is_empty())
                        .unwrap_or("unknown")
                        .to_string()
                })
                .unwrap_or_else(|| "unknown".into());
            return Ok(Some(class));
        }
    }
    Ok(None)
}

impl X11 {
    fn start(monitor: Monitor, player: Player) -> Result<X11, String> {
        let err = |e: &dyn std::fmt::Display| format!("X11: {e}");
        let (conn, screen_num) = x11rb::connect(None).map_err(|e| err(&e))?;
        let screen = conn.setup().roots[screen_num].clone();
        // 32-bit little-endian pixels for the root depth: what 0RGB is.
        let depth = screen.root_depth;
        let bpp = conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|f| f.depth == depth)
            .map(|f| f.bits_per_pixel);
        if depth != 24
            || bpp != Some(32)
            || conn.setup().image_byte_order != xproto::ImageOrder::LSB_FIRST
        {
            return Err(format!(
                "X11: a {depth}-bit screen with {bpp:?} bits per pixel isn't supported"
            ));
        }
        let monitors = conn
            .randr_get_monitors(screen.root, true)
            .map_err(|e| err(&e))?
            .reply()
            .map_err(|e| err(&e))?
            .monitors;
        if monitors.is_empty() {
            return Err("X11: no monitors".into());
        }
        let scene = player.size();
        let mut outputs = Vec::new();
        for m in &monitors {
            let (w, h) = (u32::from(m.width), u32::from(m.height));
            let (view, at) = centred_view(scene, (w, h));
            let p = player
                .with_view(view)
                .ok_or("X11: the view isn't inside the scene")?;
            let window = conn.generate_id().map_err(|e| err(&e))?;
            // Not managed by the window manager (it would tile or decorate
            // it), black under the scene, redrawn when exposed.
            conn.create_window(
                depth,
                window,
                screen.root,
                m.x,
                m.y,
                m.width,
                m.height,
                0,
                WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &CreateWindowAux::new()
                    .override_redirect(1)
                    .background_pixel(screen.black_pixel)
                    .event_mask(EventMask::EXPOSURE),
            )
            .map_err(|e| err(&e))?;
            conn.change_property8(
                xproto::PropMode::REPLACE,
                window,
                xproto::AtomEnum::WM_CLASS,
                xproto::AtomEnum::STRING,
                b"clairlune\0Clairlune\0",
            )
            .map_err(|e| err(&e))?;
            // An empty input shape: clicks go through to the root window
            // (window manager menus keep working).
            conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::INPUT,
                xproto::ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            )
            .map_err(|e| err(&e))?;
            conn.map_window(window).map_err(|e| err(&e))?;
            conn.configure_window(
                window,
                &ConfigureWindowAux::new().stack_mode(StackMode::BELOW),
            )
            .map_err(|e| err(&e))?;
            let gc = conn.generate_id().map_err(|e| err(&e))?;
            conn.create_gc(gc, window, &CreateGCAux::new())
                .map_err(|e| err(&e))?;
            outputs.push(XOutput {
                player: p,
                window,
                gc,
                at,
                exposed: true,
            });
        }
        conn.flush().map_err(|e| err(&e))?;
        let mut x = X11 {
            monitor,
            conn,
            depth,
            outputs,
        };
        x.tick()?;
        Ok(x)
    }

    fn tick(&mut self) -> Result<FrameStats, String> {
        let err = |e: &dyn std::fmt::Display| format!("X11: {e}");
        let started = Instant::now();
        while let Some(event) = self.conn.poll_for_event().map_err(|e| err(&e))? {
            if let Event::Expose(e) = event
                && let Some(o) = self.outputs.iter_mut().find(|o| o.window == e.window)
            {
                o.exposed = true;
            }
        }
        let limit = self
            .conn
            .maximum_request_bytes()
            .saturating_sub(64)
            .max(4096);
        let (mut pixels, mut count) = (0u64, 0usize);
        let mut row = Vec::new();
        for o in &mut self.outputs {
            let t = o.player.now_ms();
            let mut rects = o.player.advance(t);
            if std::mem::take(&mut o.exposed) {
                let (w, h) = o.player.size();
                rects = vec![PixelRect { x: 0, y: 0, w, h }];
            }
            for r in &rects {
                // Whole rows per request, within the request size limit.
                let rows = (limit / (r.w as usize * 4)).max(1) as u32;
                let mut y = r.y;
                while y < r.y + r.h {
                    let n = rows.min(r.y + r.h - y);
                    let part = PixelRect {
                        x: r.x,
                        y,
                        w: r.w,
                        h: n,
                    };
                    o.player.rect_pixels(&part, &mut row);
                    let bytes: Vec<u8> = row.iter().flat_map(|p| p.to_le_bytes()).collect();
                    self.conn
                        .put_image(
                            ImageFormat::Z_PIXMAP,
                            o.window,
                            o.gc,
                            r.w as u16,
                            n as u16,
                            o.at.0 + r.x as i16,
                            o.at.1 + y as i16,
                            0,
                            self.depth,
                            &bytes,
                        )
                        .map_err(|e| err(&e))?;
                    y += n;
                }
                pixels += u64::from(r.w * r.h);
            }
            count += rects.len();
        }
        self.conn.flush().map_err(|e| err(&e))?;
        Ok(FrameStats {
            pixels,
            rects: count,
            covered: 0,
            away: false,
            stalls: 0,
            busy: started.elapsed(),
            upload: started.elapsed(),
            present: Duration::ZERO,
        })
    }
}

impl Drop for X11 {
    fn drop(&mut self) {
        for o in &self.outputs {
            let _ = self.conn.free_gc(o.gc);
            let _ = self.conn.destroy_window(o.window);
        }
        let _ = self.conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktops_with_icons_keep_the_still() {
        assert_eq!(own_desktop("ubuntu:GNOME").as_deref(), Some("gnome"));
        assert_eq!(own_desktop("KDE").as_deref(), Some("kde"));
        assert_eq!(own_desktop("X-Cinnamon").as_deref(), Some("x-cinnamon"));
        assert_eq!(own_desktop("sway"), None);
        assert_eq!(own_desktop("Hyprland"), None);
        assert_eq!(own_desktop("i3"), None);
        assert_eq!(own_desktop(""), None);
    }

    #[test]
    fn a_screen_shows_the_middle_of_a_larger_scene() {
        // The scene is the largest screen's size: a smaller one shows its
        // middle, at the screen's origin.
        let (v, at) = centred_view((2560, 1440), (1920, 1080));
        assert_eq!((v.x, v.y, v.w, v.h), (320, 180, 1920, 1080));
        assert_eq!(at, (0, 0));
        // A taller screen: the whole scene width, centred vertically.
        let (v, at) = centred_view((1920, 1080), (1080, 1920));
        assert_eq!((v.x, v.y, v.w, v.h), (420, 0, 1080, 1080));
        assert_eq!(at, (0, 420));
        // The same size: all of it.
        let (v, at) = centred_view((1920, 1080), (1920, 1080));
        assert_eq!((v.x, v.y, v.w, v.h, at), (0, 0, 1920, 1080, (0, 0)));
    }
}
