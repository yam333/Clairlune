//! Compositors with the wlr layer-shell protocol (Sway, Hyprland, river,
//! Wayfire, labwc and others): Clairlune's own surfaces in the background
//! layer of every output, drawn in shared memory. They show the still
//! wallpaper where the compositor has no wallpaper tool of its own
//! ([`show_still`], held by a thread for as long as the app runs), and the
//! live wallpaper's frames ([`Surfaces`], driven by the live host).
//!
//! Every output shows the same image (Linux composes one scene, at the
//! largest output's size): the largest centred part with the output's
//! shape, scaled by the compositor to fill it (`wp_viewporter`).
//!
//! Untried: written and compile-checked without a Wayland session.

use sctk::compositor::{CompositorHandler, CompositorState};
use sctk::output::{OutputHandler, OutputState};
use sctk::reexports::client::globals::{GlobalList, registry_queue_init};
use sctk::reexports::client::protocol::{wl_output, wl_shm, wl_surface};
use sctk::reexports::client::{Connection, Dispatch, EventQueue, QueueHandle};
use sctk::reexports::protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use sctk::registry::{ProvidesRegistryState, RegistryState, SimpleGlobal};
use sctk::shell::WaylandSurface;
use sctk::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use sctk::shm::slot::{Buffer, SlotPool};
use sctk::shm::{Shm, ShmHandler};
use sctk::{
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    delegate_simple, registry_handlers,
};
use smithay_client_toolkit as sctk;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

/// The namespace compositors see for our surfaces.
const NAMESPACE: &str = "clairlune";
/// Past this many rectangles, one rectangle around them all is cheaper.
const MAX_RECTS: usize = 64;

/// A rectangle of the image, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// An output, as the compositor describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// The connector name (`DP-1`), or the compositor's id for it.
    pub name: String,
    /// Physical pixels.
    pub width: u32,
    pub height: u32,
}

/// The part of an `image`-sized picture an `output`-shaped screen shows
/// when filled: the largest centred crop with the output's shape, as
/// `(x, y, w, h)` in image pixels.
pub fn fill_crop(image: (u32, u32), output: (u32, u32)) -> (f64, f64, f64, f64) {
    let (iw, ih) = (f64::from(image.0), f64::from(image.1));
    let (ow, oh) = (f64::from(output.0.max(1)), f64::from(output.1.max(1)));
    let scale = (iw / ow).min(ih / oh);
    let (w, h) = (ow * scale, oh * scale);
    ((iw - w) / 2.0, (ih - h) / 2.0, w, h)
}

/// Adds `more` to `list`, keeping it to at most [`MAX_RECTS`] (and the
/// size of `more`): a surface the compositor isn't asking frames for (an
/// output that's off) gathers damage for a long time.
fn add_damage(list: &mut Vec<Rect>, more: &[Rect]) {
    list.extend_from_slice(more);
    if list.len() > MAX_RECTS {
        *list = bounded(std::mem::take(list));
    }
}

/// `rects` as at most [`MAX_RECTS`] rectangles: themselves, or the one
/// around them all.
fn bounded(rects: Vec<Rect>) -> Vec<Rect> {
    if rects.len() <= MAX_RECTS {
        return rects;
    }
    let x0 = rects.iter().map(|r| r.x).min().unwrap_or(0);
    let y0 = rects.iter().map(|r| r.y).min().unwrap_or(0);
    let x1 = rects.iter().map(|r| r.x + r.w).max().unwrap_or(0);
    let y1 = rects.iter().map(|r| r.y + r.h).max().unwrap_or(0);
    vec![Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }]
}

/// Each output's size in pixels, largest first, from the compositor (for
/// the size to render at).
pub fn output_sizes() -> Result<Vec<(u32, u32)>, String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("Wayland: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<State>(&conn).map_err(|e| format!("Wayland: {e}"))?;
    let qh = queue.handle();
    let mut state = bind(&globals, &qh, (0, 0))?;
    for _ in 0..2 {
        queue
            .roundtrip(&mut state)
            .map_err(|e| format!("Wayland: {e}"))?;
    }
    let mut sizes: Vec<(u32, u32)> = state
        .outputs
        .outputs()
        .filter(|o| state.outputs.info(o).is_some())
        .map(|o| state.physical(&o))
        .filter(|&(w, h)| w > 1 && h > 1)
        .collect();
    sizes.sort_by_key(|&(w, h)| std::cmp::Reverse(u64::from(w) * u64::from(h)));
    Ok(sizes)
}

/// Whether this session can show our background surfaces: Wayland, with
/// layer shell, shared memory and the viewporter.
pub fn available() -> Result<(), String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err("not a Wayland session".into());
    }
    let conn = Connection::connect_to_env().map_err(|e| format!("Wayland: {e}"))?;
    let (globals, _queue) =
        registry_queue_init::<Probe>(&conn).map_err(|e| format!("Wayland: {e}"))?;
    let names: Vec<String> = globals
        .contents()
        .with_list(|l| l.iter().map(|g| g.interface.clone()).collect());
    for needed in ["zwlr_layer_shell_v1", "wl_shm", "wp_viewporter"] {
        if !names.iter().any(|n| n == needed) {
            return Err(format!("the compositor has no `{needed}`"));
        }
    }
    Ok(())
}

/// Registry events while probing: nothing to do.
struct Probe;
impl
    Dispatch<
        sctk::reexports::client::protocol::wl_registry::WlRegistry,
        sctk::reexports::client::globals::GlobalListContents,
    > for Probe
{
    fn event(
        _: &mut Self,
        _: &sctk::reexports::client::protocol::wl_registry::WlRegistry,
        _: sctk::reexports::client::protocol::wl_registry::Event,
        _: &sctk::reexports::client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// One buffer of an output's surface, and the parts of the image it lacks.
struct Slotted {
    buffer: Buffer,
    stale: Vec<Rect>,
}

/// One output's background surface.
struct Entry {
    output: wl_output::WlOutput,
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    /// Logical size from the last configure; `None` until the first.
    size: Option<(u32, u32)>,
    /// Physical size, for the crop's shape.
    physical: (u32, u32),
    /// A frame callback is out: the compositor hasn't asked for more yet.
    waiting: bool,
    /// Changed since the last commit (what to damage), and whether it has
    /// never been drawn.
    damage: Vec<Rect>,
    fresh: bool,
    buffers: Vec<Slotted>,
}

impl Drop for Entry {
    /// The viewport isn't the surface's: it goes away only when asked.
    fn drop(&mut self) {
        self.viewport.destroy();
    }
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    viewporter: SimpleGlobal<wp_viewporter::WpViewporter, 1>,
    entries: Vec<Entry>,
    image: (u32, u32),
    /// The first outputs have their surfaces: one that comes later (plugged
    /// in, or back after every screen went away) gets its own.
    started: bool,
}

impl State {
    fn physical(&self, output: &wl_output::WlOutput) -> (u32, u32) {
        let Some(info) = self.outputs.info(output) else {
            return self.image;
        };
        let mode = info
            .modes
            .iter()
            .find(|m| m.current)
            .map(|m| m.dimensions)
            .or_else(|| {
                info.logical_size
                    .map(|(w, h)| (w * info.scale_factor, h * info.scale_factor))
            })
            .unwrap_or((self.image.0 as i32, self.image.1 as i32));
        let (w, h) = (mode.0.max(1) as u32, mode.1.max(1) as u32);
        // Modes are given unrotated.
        match info.transform {
            wl_output::Transform::_90
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped270 => (h, w),
            _ => (w, h),
        }
    }

    fn add(&mut self, qh: &QueueHandle<State>, output: wl_output::WlOutput) {
        if self.entries.iter().any(|e| e.output == output) {
            return;
        }
        let Ok(viewporter) = self.viewporter.get() else {
            return;
        };
        let surface = self.compositor.create_surface(qh);
        let viewport = viewporter.get_viewport(&surface, qh, ());
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Background,
            Some(NAMESPACE),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        // Under panels too, like any wallpaper.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_size(0, 0);
        // An empty input region: clicks go to the desktop below.
        if let Ok(region) = sctk::compositor::Region::new(&self.compositor) {
            layer
                .wl_surface()
                .set_input_region(Some(region.wl_region()));
        }
        layer.commit();
        let physical = self.physical(&output);
        self.entries.push(Entry {
            output,
            layer,
            viewport,
            size: None,
            physical,
            waiting: false,
            damage: Vec::new(),
            fresh: true,
            buffers: Vec::new(),
        });
    }
}

/// Background surfaces on every output, showing one image.
pub struct Surfaces {
    queue: EventQueue<State>,
    state: State,
    pool: SlotPool,
}

impl Surfaces {
    /// Opens a background surface on every output, for an image of `size`
    /// pixels. Nothing shows until [`Surfaces::present`].
    pub fn new(size: (u32, u32)) -> Result<Surfaces, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("Wayland: {e}"))?;
        let (globals, queue) =
            registry_queue_init::<State>(&conn).map_err(|e| format!("Wayland: {e}"))?;
        let qh = queue.handle();
        let state = bind(&globals, &qh, size)?;
        let pool = SlotPool::new((size.0 * size.1 * 4) as usize, &state.shm)
            .map_err(|e| format!("shared memory: {e}"))?;
        let mut s = Surfaces { queue, state, pool };
        // Output descriptions arrive with the first round trips; then a
        // surface per output, then their first configures.
        for _ in 0..2 {
            s.queue
                .roundtrip(&mut s.state)
                .map_err(|e| format!("Wayland: {e}"))?;
        }
        let outputs: Vec<_> = s.state.outputs.outputs().collect();
        for o in outputs {
            s.state.add(&qh, o);
        }
        s.state.started = true;
        s.queue
            .roundtrip(&mut s.state)
            .map_err(|e| format!("Wayland: {e}"))?;
        if s.state.entries.is_empty() {
            return Err("no outputs".into());
        }
        Ok(s)
    }

    /// The image size these surfaces show.
    pub fn image_size(&self) -> (u32, u32) {
        self.state.image
    }

    /// The outputs shown on.
    pub fn outputs(&self) -> Vec<Output> {
        self.state
            .entries
            .iter()
            .map(|e| {
                let info = self.state.outputs.info(&e.output);
                Output {
                    name: info
                        .as_ref()
                        .and_then(|i| i.name.clone())
                        .unwrap_or_else(|| format!("wl-output-{}", info.map_or(0, |i| i.id))),
                    width: e.physical.0,
                    height: e.physical.1,
                }
            })
            .collect()
    }

    /// Sends what is queued, then reads and handles what the compositor
    /// sent, waiting at most `wait` for it.
    pub fn dispatch(&mut self, wait: Duration) -> Result<(), String> {
        self.queue.flush().map_err(|e| format!("Wayland: {e}"))?;
        if let Some(guard) = self.queue.prepare_read() {
            let mut fd = libc::pollfd {
                fd: guard.connection_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ms = wait.as_millis().min(i32::MAX as u128) as i32;
            // SAFETY: one valid pollfd for the duration of the call.
            let ready = unsafe { libc::poll(&mut fd, 1, ms) };
            if ready > 0 {
                guard.read().map_err(|e| format!("Wayland: {e}"))?;
            }
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(|e| format!("Wayland: {e}"))?;
        Ok(())
    }

    /// Outputs still waiting for their first frame.
    pub fn pending(&self) -> usize {
        self.state.entries.iter().filter(|e| e.fresh).count()
    }

    /// The image changed in `changed`: shows it on every output that is
    /// ready (configured, and asked for a frame). `fill` writes one
    /// rectangle of the current image into a canvas of the image's size,
    /// XRGB8888 little-endian with the given stride in bytes. Returns the
    /// outputs drawn.
    pub fn present(
        &mut self,
        changed: &[Rect],
        fill: &mut dyn FnMut(Rect, &mut [u8], usize),
    ) -> Result<usize, String> {
        let qh = self.queue.handle();
        let (iw, ih) = self.state.image;
        let whole = Rect {
            x: 0,
            y: 0,
            w: iw,
            h: ih,
        };
        let stride = iw as usize * 4;
        let mut drawn = 0;
        for e in &mut self.state.entries {
            add_damage(&mut e.damage, changed);
            for b in &mut e.buffers {
                add_damage(&mut b.stale, changed);
            }
            let Some((lw, lh)) = e.size else { continue };
            if e.waiting || (e.damage.is_empty() && !e.fresh) {
                continue;
            }
            // A buffer the compositor has let go of, or a new one.
            let free = e
                .buffers
                .iter()
                .position(|b| b.buffer.canvas(&mut self.pool).is_some());
            let i = match free {
                Some(i) => i,
                None if e.buffers.len() < 2 => {
                    let (buffer, _) = self
                        .pool
                        .create_buffer(
                            iw as i32,
                            ih as i32,
                            stride as i32,
                            wl_shm::Format::Xrgb8888,
                        )
                        .map_err(|err| format!("shared memory: {err}"))?;
                    e.buffers.push(Slotted {
                        buffer,
                        stale: vec![whole],
                    });
                    e.buffers.len() - 1
                }
                None => continue, // both still on screen: next time
            };
            let slot = &mut e.buffers[i];
            let canvas = slot
                .buffer
                .canvas(&mut self.pool)
                .ok_or("a free buffer has no canvas")?;
            for r in bounded(std::mem::take(&mut slot.stale)) {
                fill(r, canvas, stride);
            }
            let surface = e.layer.wl_surface();
            let damage = if e.fresh {
                vec![whole]
            } else {
                bounded(std::mem::take(&mut e.damage))
            };
            for r in damage {
                surface.damage_buffer(r.x as i32, r.y as i32, r.w as i32, r.h as i32);
            }
            e.damage.clear();
            let (sx, sy, sw, sh) = fill_crop((iw, ih), e.physical);
            e.viewport.set_source(sx, sy, sw, sh);
            e.viewport.set_destination(lw as i32, lh as i32);
            surface.frame(&qh, surface.clone());
            slot.buffer
                .attach_to(surface)
                .map_err(|err| format!("attach: {err:?}"))?;
            e.layer.commit();
            e.waiting = true;
            e.fresh = false;
            drawn += 1;
        }
        self.queue.flush().map_err(|e| format!("Wayland: {e}"))?;
        Ok(drawn)
    }
}

fn bind(globals: &GlobalList, qh: &QueueHandle<State>, image: (u32, u32)) -> Result<State, String> {
    Ok(State {
        registry: RegistryState::new(globals),
        outputs: OutputState::new(globals, qh),
        compositor: CompositorState::bind(globals, qh)
            .map_err(|e| format!("wl_compositor: {e}"))?,
        layer_shell: LayerShell::bind(globals, qh)
            .map_err(|e| format!("the compositor has no layer shell: {e}"))?,
        shm: Shm::bind(globals, qh).map_err(|e| format!("wl_shm: {e}"))?,
        viewporter: SimpleGlobal::bind(globals, qh).map_err(|e| format!("wp_viewporter: {e}"))?,
        entries: Vec::new(),
        image,
        started: false,
    })
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.layer.wl_surface() == surface)
        {
            e.waiting = false;
        }
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    /// A screen plugged in after the start gets its surface too.
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if self.started {
            self.add(qh, output);
        }
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let physical = self.physical(&output);
        if let Some(e) = self.entries.iter_mut().find(|e| e.output == output) {
            e.physical = physical;
            e.fresh = true;
        }
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.entries.retain(|e| e.output != output);
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.entries.retain(|e| &e.layer != layer);
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        // 0 means "pick one": the output's own size.
        let logical = self
            .entries
            .iter()
            .find(|e| &e.layer == layer)
            .and_then(|e| self.outputs.info(&e.output))
            .and_then(|i| i.logical_size)
            .map(|(w, h)| (w.max(1) as u32, h.max(1) as u32));
        if let Some(e) = self.entries.iter_mut().find(|e| &e.layer == layer) {
            let (w, h) = configure.new_size;
            let (lw, lh) = logical.unwrap_or(e.physical);
            let size = (if w == 0 { lw } else { w }, if h == 0 { lh } else { h });
            if e.size != Some(size) {
                e.size = Some(size);
                e.fresh = true;
            }
        }
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}

impl AsMut<SimpleGlobal<wp_viewporter::WpViewporter, 1>> for State {
    fn as_mut(&mut self) -> &mut SimpleGlobal<wp_viewporter::WpViewporter, 1> {
        &mut self.viewporter
    }
}

impl Dispatch<wp_viewport::WpViewport, ()> for State {
    fn event(
        _: &mut Self,
        _: &wp_viewport::WpViewport,
        _: wp_viewport::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // wp_viewport has no events.
    }
}

delegate_compositor!(State);
delegate_output!(State);
delegate_shm!(State);
delegate_layer!(State);
delegate_registry!(State);
delegate_simple!(State, wp_viewporter::WpViewporter, 1);

// ---- The still wallpaper, held by a thread.

/// An image as XRGB8888 pixels.
struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

enum Command {
    Show(Image),
    Stop,
}

struct StillThread {
    tx: mpsc::Sender<Command>,
}

static STILL: Mutex<Option<StillThread>> = Mutex::new(None);

/// Decodes a PNG (any colour type) to XRGB8888.
fn decode(path: &Path) -> Result<Image, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec
        .read_info()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let channels = info.color_type.samples();
    let pixels = buf[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|p| match channels {
            1 | 2 => u32::from(p[0]) * 0x01_0101,
            _ => (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2]),
        })
        .collect();
    Ok(Image {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// Shows a still image on every output, in the background layer, until
/// [`hide_still`] or the process ends. Returns once the surfaces exist.
pub fn show_still(path: &Path) -> Result<(), String> {
    let image = decode(path)?;
    let mut held = STILL.lock().map_err(|_| "the still thread panicked")?;
    if let Some(t) = held.as_ref() {
        match t.tx.send(Command::Show(image)) {
            Ok(()) => return Ok(()),
            // The thread ended (the compositor went away): start again.
            Err(mpsc::SendError(Command::Show(img))) => return start(&mut held, img),
            Err(_) => unreachable!("only Show is sent here"),
        }
    }
    start(&mut held, image)
}

/// Takes the still surfaces away (what the compositor draws itself shows).
pub fn hide_still() {
    if let Ok(mut held) = STILL.lock()
        && let Some(t) = held.take()
    {
        let _ = t.tx.send(Command::Stop);
    }
}

fn start(held: &mut Option<StillThread>, image: Image) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("clairlune-wlr-still".into())
        .spawn(move || still_loop(image, &rx, &ready_tx))
        .map_err(|e| format!("still thread: {e}"))?;
    ready_rx
        .recv()
        .map_err(|_| "the still thread ended".to_string())??;
    *held = Some(StillThread { tx });
    Ok(())
}

fn still_loop(
    first: Image,
    rx: &mpsc::Receiver<Command>,
    ready: &mpsc::Sender<Result<(), String>>,
) {
    let open = |img: &Image| Surfaces::new((img.width, img.height));
    let mut surfaces = match open(&first) {
        Ok(s) => {
            let _ = ready.send(Ok(()));
            s
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let mut image = first;
    let mut changed = true;
    loop {
        match rx.try_recv() {
            Ok(Command::Show(img)) => {
                if (img.width, img.height) != surfaces.image_size() {
                    match open(&img) {
                        Ok(s) => surfaces = s,
                        Err(e) => {
                            tracing::warn!(target: "clairlune_platform", error = %e, "still surfaces");
                            return;
                        }
                    }
                }
                image = img;
                changed = true;
            }
            Ok(Command::Stop) | Err(mpsc::TryRecvError::Disconnected) => return,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if let Err(e) = surfaces.dispatch(Duration::from_millis(250)) {
            tracing::warn!(target: "clairlune_platform", error = %e, "still surfaces lost");
            return;
        }
        let whole = Rect {
            x: 0,
            y: 0,
            w: image.width,
            h: image.height,
        };
        let rects = if changed { vec![whole] } else { Vec::new() };
        let img = &image;
        let result = surfaces.present(&rects, &mut |r, canvas, stride| {
            copy_rect(&img.pixels, img.width, r, canvas, stride);
        });
        if let Err(e) = result {
            tracing::warn!(target: "clairlune_platform", error = %e, "still surfaces");
            return;
        }
        changed = false;
    }
}

/// Copies rectangle `r` of an XRGB image `width` pixels wide into a canvas
/// of the same size.
pub fn copy_rect(pixels: &[u32], width: u32, r: Rect, canvas: &mut [u8], stride: usize) {
    for y in r.y..r.y + r.h {
        let src = &pixels[(y * width + r.x) as usize..][..r.w as usize];
        let dst = &mut canvas[y as usize * stride + r.x as usize * 4..][..r.w as usize * 4];
        for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src) {
            d.copy_from_slice(&s.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wider_screen_shows_a_band_across_the_middle() {
        // A 16:9 image on a 21:9 screen: full width, a centred band.
        let (x, y, w, h) = fill_crop((1920, 1080), (2560, 1080));
        assert_eq!((x, w), (0.0, 1920.0));
        assert!((h - 810.0).abs() < 1e-9 && (y - 135.0).abs() < 1e-9);
        // The same shape: everything.
        assert_eq!(
            fill_crop((1920, 1080), (3840, 2160)),
            (0.0, 0.0, 1920.0, 1080.0)
        );
        // Portrait: a centred column.
        let (x, _, w, h) = fill_crop((1920, 1080), (1080, 1920));
        assert_eq!(h, 1080.0);
        assert!((w - 607.5).abs() < 1e-9 && (x - 656.25).abs() < 1e-9);
    }

    #[test]
    fn damage_waiting_for_a_frame_stays_bounded() {
        let r = |x| Rect {
            x,
            y: 0,
            w: 1,
            h: 1,
        };
        let mut list = Vec::new();
        for i in 0..10_000 {
            add_damage(&mut list, &[r(i), r(i + 1)]);
            assert!(list.len() <= MAX_RECTS + 2);
        }
        let all = bounded(list);
        assert_eq!(all.len(), 1);
        assert_eq!((all[0].x, all[0].w), (0, 10_001));
    }

    #[test]
    fn many_rects_become_one_around_them() {
        let few = vec![
            Rect {
                x: 1,
                y: 2,
                w: 3,
                h: 4
            };
            3
        ];
        assert_eq!(bounded(few.clone()), few);
        let many: Vec<Rect> = (0..100)
            .map(|i| Rect {
                x: i,
                y: 10 + i,
                w: 2,
                h: 1,
            })
            .collect();
        assert_eq!(
            bounded(many),
            vec![Rect {
                x: 0,
                y: 10,
                w: 101,
                h: 100
            }]
        );
    }

    #[test]
    fn rects_copy_as_little_endian_xrgb() {
        let pixels = vec![
            0x00_11_22_33u32,
            0x00_44_55_66,
            0x00_77_88_99,
            0x00_AA_BB_CC,
        ];
        let mut canvas = vec![0u8; 16];
        copy_rect(
            &pixels,
            2,
            Rect {
                x: 1,
                y: 1,
                w: 1,
                h: 1,
            },
            &mut canvas,
            8,
        );
        assert_eq!(&canvas[12..16], &[0xCC, 0xBB, 0xAA, 0x00]);
        assert!(canvas[..12].iter().all(|&b| b == 0));
    }
}
