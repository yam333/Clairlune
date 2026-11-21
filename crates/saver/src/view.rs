//! `ClairluneSaverView`: the screen saver's view, one per screen (and one
//! small one for the preview in System Settings).

use clairlune_live::Player;
use clairlune_live::gpu::{Canvas, Gpu, exact_format};
use clairlune_render::ScenePackage;
use clairlune_scene::saver::{SAVER_SCENE_FILE, SaverScene};
use clairlune_scene::{Aspect, ContentContext, Theme, compose};
use objc2::rc::{Allocated, Retained};
use objc2::{ClassType, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSResponder, NSView};
use objc2_foundation::{NSObject, NSRect};
use objc2_screen_saver::ScreenSaverView;
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Arc;

/// What a view plays, once it starts. Fields drop in order: the surface
/// before the device and the instance.
struct Playing {
    player: Player,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    canvas: Canvas,
    gpu: Gpu,
    _instance: wgpu::Instance,
}

#[derive(Default)]
struct Ivars {
    playing: RefCell<Option<Playing>>,
}

define_class!(
    // SAFETY: ScreenSaverView asks nothing of subclasses beyond calling its
    // initialiser, which `initWithFrame:isPreview:` does; there is no Drop.
    #[unsafe(super(ScreenSaverView, NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ClairluneSaverView"]
    #[ivars = Ivars]
    struct ClairluneSaverView;

    impl ClairluneSaverView {
        #[unsafe(method_id(initWithFrame:isPreview:))]
        fn init_with_frame(
            this: Allocated<Self>,
            frame: NSRect,
            is_preview: bool,
        ) -> Option<Retained<Self>> {
            let this = this.set_ivars(Ivars::default());
            let this: Option<Retained<Self>> =
                unsafe { msg_send![super(this), initWithFrame: frame, isPreview: is_preview] };
            if let Some(view) = &this {
                unsafe { view.setAnimationTimeInterval(1.0 / 12.0) };
            }
            this
        }

        #[unsafe(method(startAnimation))]
        fn start_animation(&self) {
            let _: () = unsafe { msg_send![super(self), startAnimation] };
            self.start();
        }

        #[unsafe(method(stopAnimation))]
        fn stop_animation(&self) {
            let _: () = unsafe { msg_send![super(self), stopAnimation] };
            self.ivars().playing.borrow_mut().take();
        }

        #[unsafe(method(animateOneFrame))]
        fn animate_one_frame(&self) {
            self.next_frame();
        }

        #[unsafe(method(hasConfigureSheet))]
        fn has_configure_sheet(&self) -> bool {
            false
        }
    }
);

/// The screen saver host finds the principal class by name as soon as the
/// bundle loads, before anything here runs: registered from the library's
/// initialiser.
extern "C" fn register() {
    let _ = ClairluneSaverView::class();
}

#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
static REGISTER: extern "C" fn() = register;

impl ClairluneSaverView {
    fn start(&self) {
        if self.ivars().playing.borrow().is_some() {
            return;
        }
        match self.make() {
            Ok(p) => *self.ivars().playing.borrow_mut() = Some(p),
            Err(e) => eprintln!("Clairlune screen saver: {e}"),
        }
    }

    /// The view's size in pixels.
    fn pixel_size(&self) -> (u32, u32) {
        let scale = self.window().map_or(2.0, |w| w.backingScaleFactor());
        let b = self.bounds();
        (
            (b.size.width * scale).round().max(0.0) as u32,
            (b.size.height * scale).round().max(0.0) as u32,
        )
    }

    fn make(&self) -> Result<Playing, String> {
        let (w, h) = self.pixel_size();
        if w < 16 || h < 16 {
            return Err(format!("the view is too small ({w}x{h})"));
        }
        let picked = current_scene().and_then(|s| {
            s.for_size(w, h)
                .map(|m| (m.recipe.clone(), s.theme, s.buddy, s.fps))
        });
        let (recipe, theme, buddy, fps) = picked.unwrap_or_else(|| {
            // Nothing from the app: a scene of our own.
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64);
            let r = compose(seed, Aspect::from_size(w, h), &ContentContext::default());
            (r, Theme::Dark, true, 12)
        });
        let pkg = Arc::new(ScenePackage::capture(&recipe, w, h).map_err(|e| e.to_string())?);
        let theme = if pkg.themes().contains(&theme) {
            theme
        } else {
            pkg.themes()[0]
        };
        let mut player = Player::new(pkg, theme, buddy).ok_or("the scene has no theme")?;
        player.set_smooth(true);
        unsafe { self.setAnimationTimeInterval(1.0 / f64::from(fps.clamp(1, 30))) };

        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(desc);
        let view: &NSView = self;
        let handle = AppKitWindowHandle::new(NonNull::from(view).cast());
        // SAFETY: the surface is dropped (with `Playing`) before the view,
        // which owns it through its ivars.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
                raw_window_handle: RawWindowHandle::AppKit(handle),
            })
        }
        .map_err(|e| e.to_string())?;
        let gpu = Gpu::new(&instance, Some(&surface), None, false)?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = exact_format(&caps.formats)
            .ok_or_else(|| format!("no 8-bit surface format in {:?}", caps.formats))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 1,
            alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                wgpu::CompositeAlphaMode::Opaque
            } else {
                wgpu::CompositeAlphaMode::Auto
            },
            view_formats: vec![],
            color_space: Default::default(),
        };
        surface.configure(&gpu.device, &config);
        let t = player.now_ms();
        player.advance(t);
        let canvas = gpu.canvas(&player);
        let mut playing = Playing {
            player,
            surface,
            config,
            canvas,
            gpu,
            _instance: instance,
        };
        draw(&mut playing);
        Ok(playing)
    }

    fn next_frame(&self) {
        let mut slot = self.ivars().playing.borrow_mut();
        let Some(p) = slot.as_mut() else {
            return;
        };
        let t = p.player.now_ms();
        let rects = p.player.advance(t);
        if rects.is_empty() {
            return;
        }
        p.gpu.upload(&mut p.canvas, &p.player, &rects);
        draw(p);
    }
}

fn draw(p: &mut Playing) {
    let frame = match p.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            p.surface.configure(&p.gpu.device, &p.config);
            return;
        }
        _ => return,
    };
    let view = frame.texture.create_view(&Default::default());
    p.gpu.draw(&p.canvas, &view, p.config.format);
    p.gpu.queue.present(frame);
}

/// The scene the app last set, from its data folder (the user's real home:
/// the host gives savers a home of their own) or from this bundle's
/// Resources, where the app copies it too.
fn current_scene() -> Option<SaverScene> {
    let paths = [
        real_home().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("clairlune")
                .join(SAVER_SCENE_FILE)
        }),
        bundle_resources().map(|r| r.join(SAVER_SCENE_FILE)),
    ];
    paths.into_iter().flatten().find_map(|p| {
        let text = std::fs::read_to_string(p).ok()?;
        serde_json::from_str(&text).ok()
    })
}

fn real_home() -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: getpwuid returns a pointer into static storage (or null),
    // read at once.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_dir.is_null() {
            return None;
        }
        let dir = std::ffi::CStr::from_ptr((*pw).pw_dir);
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes())))
    }
}

/// `Clairlune.saver/Contents/Resources`, from where this library is.
fn bundle_resources() -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: dladdr fills `info` for an address in a loaded image; the
    // name is copied out at once.
    unsafe {
        let mut info: libc::Dl_info = std::mem::zeroed();
        let here = register as *const std::ffi::c_void;
        if libc::dladdr(here, &mut info) == 0 || info.dli_fname.is_null() {
            return None;
        }
        let path = std::ffi::CStr::from_ptr(info.dli_fname);
        let exe = PathBuf::from(std::ffi::OsStr::from_bytes(path.to_bytes()));
        Some(exe.parent()?.parent()?.join("Resources"))
    }
}
