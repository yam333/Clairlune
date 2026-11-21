//! `ClairluneSaverView`: the screen saver's view, one per screen (and one
//! small one for the preview in System Settings). It shows the scene like
//! the live wallpaper's windows, through IOSurfaces in a view of its own
//! ([`clairlune_live::surface`]) filling it.

use clairlune_live::Player;
use clairlune_live::surface::SurfaceView;
use clairlune_render::ScenePackage;
use clairlune_scene::saver::{SAVER_SCENE_FILE, SaverScene};
use clairlune_scene::{Aspect, ContentContext, Theme, compose};
use objc2::rc::{Allocated, Retained};
use objc2::{ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSResponder, NSView};
use objc2_foundation::{NSDistributedNotificationCenter, NSObject, NSRect, NSString};
use objc2_screen_saver::ScreenSaverView;
use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Once};

#[derive(Default)]
struct Ivars {
    playing: RefCell<Option<SurfaceView>>,
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
                if !is_preview {
                    quit_when_the_saver_stops();
                }
                log(&format!(
                    "view {}x{} points, preview {is_preview}",
                    frame.size.width, frame.size.height
                ));
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
            if let Some(p) = self.ivars().playing.borrow_mut().take() {
                p.view().removeFromSuperview();
            }
        }

        #[unsafe(method(animateOneFrame))]
        fn animate_one_frame(&self) {
            self.next_frame();
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            true
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

/// macOS 14 and later keep a saver's host running after the screen saver
/// ends (its views keep animating, unseen): it says the saver will stop,
/// and the host quits then, the way other screen savers do it. The next
/// screen saver starts a new host.
fn quit_when_the_saver_stops() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let name = NSString::from_str("com.apple.screensaver.willstop");
        let block = block2::RcBlock::new(|_| {
            log("the screen saver stops; quitting");
            std::process::exit(0);
        });
        let center = NSDistributedNotificationCenter::defaultCenter();
        // SAFETY: the name is a valid NSString; the block takes the
        // notification and asks nothing of it.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(&name), None, None, &block)
        };
        // Observed until the host quits.
        std::mem::forget(token);
    });
}

impl ClairluneSaverView {
    fn start(&self) {
        if self.ivars().playing.borrow().is_some() {
            return;
        }
        match self.make() {
            Ok(p) => {
                // Filling this view, as it resizes.
                p.view().setAutoresizingMask(
                    NSAutoresizingMaskOptions::ViewWidthSizable
                        | NSAutoresizingMaskOptions::ViewHeightSizable,
                );
                self.addSubview(p.view());
                *self.ivars().playing.borrow_mut() = Some(p);
            }
            Err(e) => log(&format!("not playing: {e}")),
        }
    }

    /// The view's size in pixels.
    fn pixel_size(&self) -> (u32, u32) {
        let b = self.convertRectToBacking(self.bounds());
        (
            b.size.width.round().max(0.0) as u32,
            b.size.height.round().max(0.0) as u32,
        )
    }

    fn make(&self) -> Result<SurfaceView, String> {
        let (w, h) = self.pixel_size();
        if w < 16 || h < 16 {
            return Err(format!("the view is too small ({w}x{h})"));
        }
        let picked = current_scene().and_then(|(s, from)| {
            s.for_size(w, h)
                .map(|m| (m.recipe.clone(), s.theme, s.buddy, s.fps, from))
        });
        let (recipe, theme, buddy, fps) = match picked {
            Some((r, theme, buddy, fps, from)) => {
                log(&format!("{w}x{h}: the app's scene, from {from}"));
                (r, theme, buddy, fps)
            }
            None => {
                // Nothing from the app for this size: a scene of our own.
                log(&format!("{w}x{h}: a scene of its own"));
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64);
                let r = compose(seed, Aspect::from_size(w, h), &ContentContext::default());
                (r, Theme::Dark, true, 12)
            }
        };
        let pkg = Arc::new(ScenePackage::capture(&recipe, w, h).map_err(|e| e.to_string())?);
        let theme = if pkg.themes().contains(&theme) {
            theme
        } else {
            pkg.themes()[0]
        };
        let mut player = Player::new(pkg, theme, buddy).ok_or("the scene has no theme")?;
        player.set_smooth(true);
        let t = player.now_ms();
        player.advance(t);
        unsafe { self.setAnimationTimeInterval(1.0 / f64::from(fps.clamp(1, 30))) };
        let mtm = MainThreadMarker::from(self);
        SurfaceView::new(mtm, self.bounds(), player).ok_or_else(|| "no IOSurface".to_string())
    }

    fn next_frame(&self) {
        if let Some(p) = self.ivars().playing.borrow_mut().as_mut() {
            p.advance();
        }
    }
}

/// The scene the app last set, and where it came from: the app's data
/// folder (the user's real home: the host gives savers a home of their own,
/// and its sandbox may keep that folder out) or this bundle's Resources,
/// where the app copies it too.
fn current_scene() -> Option<(SaverScene, &'static str)> {
    let paths = [
        (
            real_home().map(|h| {
                h.join("Library")
                    .join("Application Support")
                    .join("clairlune")
                    .join(SAVER_SCENE_FILE)
            }),
            "the app's folder",
        ),
        (
            bundle_resources().map(|r| r.join(SAVER_SCENE_FILE)),
            "the saver's Resources",
        ),
    ];
    paths.into_iter().find_map(|(p, from)| {
        let text = std::fs::read_to_string(p?).ok()?;
        Some((serde_json::from_str(&text).ok()?, from))
    })
}

/// A line in `~/Library/Logs/Clairlune Saver.log` of the home the host
/// gives the saver (inside its container: on macOS 14,
/// `~/Library/Containers/com.apple.ScreenSaver.Engine.legacyScreenSaver/Data`).
fn log(line: &str) {
    let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) else {
        return;
    };
    let dir = PathBuf::from(home).join("Library").join("Logs");
    let path = dir.join("Clairlune Saver.log");
    let _ = std::fs::create_dir_all(&dir);
    // Kept small: started over past 256 KB.
    let fresh = std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(!fresh)
        .write(true)
        .truncate(fresh)
        .open(&path);
    if let Ok(mut f) = file {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(
            f,
            "{secs} [{}] {} {line}",
            std::process::id(),
            env!("CARGO_PKG_VERSION")
        );
    }
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
