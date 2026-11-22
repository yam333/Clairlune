//! `preview-activity`: a developer window that plays one activity in the
//! scenes it fits, and reloads the activity files and the sprite sheets when
//! they change on disk (the authoring loop: draw, save, see it in a real
//! wallpaper). With `--export`, saving an Aseprite source or
//! a design script also re-exports the sheets.
//!
//! Keys: N / Right next scene, P / Left previous, T theme, B Buddy, R reload,
//! Esc close. Errors in the files go to the terminal; the window keeps the
//! last scene that loaded.

use crate::ThemeArg;
use crate::gifcmd::SmoothnessArg;
use clairlune_live::Player;
use clairlune_render::ScenePackage;
use clairlune_scene::activity::Catalog;
use clairlune_scene::compose::compose_with;
use clairlune_scene::{Aspect, ContentContext, Theme};
use clairlune_sprites::SheetCharacter;
use clap::Args;
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

#[derive(Args)]
pub struct PreviewArgs {
    /// Activity id (e.g. `warm_hands`).
    activity: String,
    /// The first scene's seed; N and P step to the next and previous seeds.
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, value_enum, default_value_t = ThemeArg::Dark)]
    theme: ThemeArg,
    /// Frame rate, as the Smoothness setting names it.
    #[arg(long, value_enum, default_value_t = SmoothnessArg::Smooth)]
    smoothness: SmoothnessArg,
    /// Window size in pixels.
    #[arg(long, default_value = "1280x720", value_parser = crate::parse_size)]
    size: (u32, u32),
    /// The repository to watch. Found from the current folder when omitted.
    #[arg(long)]
    root: Option<PathBuf>,
    /// When an Aseprite source or design script changes, rebuild and export
    /// it with scripts/export_sprites.py (needs Aseprite).
    #[arg(long)]
    export: bool,
    /// Close after this many seconds (for scripted checks).
    #[arg(long, hide = true)]
    exit_after: Option<f64>,
}

fn fps(s: SmoothnessArg) -> u32 {
    match s {
        SmoothnessArg::BatterySaver => 6,
        SmoothnessArg::Balanced => 12,
        SmoothnessArg::Smooth => 24,
        SmoothnessArg::VerySmooth => 30,
        SmoothnessArg::UltraSmooth => 60,
    }
}

// ---- Sources on disk -----------------------------------------------------------

/// The repository folders the preview reads and watches.
pub(crate) struct Sources {
    root: PathBuf,
}

/// Modification time and length of every watched file.
type Stamps = BTreeMap<PathBuf, (SystemTime, u64)>;

impl Sources {
    /// `root`, or the first folder up from the current one that holds the
    /// sprite sheets, or the repository this binary was built from.
    pub(crate) fn find(root: Option<&Path>) -> Result<Sources, String> {
        let is_repo = |p: &Path| p.join("crates/sprites/assets/clawd").is_dir();
        if let Some(r) = root {
            return if is_repo(r) {
                Ok(Sources {
                    root: r.to_path_buf(),
                })
            } else {
                Err(format!("{} is not a Clairlune repository", r.display()))
            };
        }
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        cwd.ancestors()
            .find(|p| is_repo(p))
            .map(Path::to_path_buf)
            .or_else(|| is_repo(&built).then_some(built))
            .map(|root| Sources { root })
            .ok_or_else(|| "run this inside the Clairlune repository, or pass --root".to_string())
    }

    fn activities(&self) -> PathBuf {
        self.root.join("crates/scene/activities")
    }

    fn sheets(&self) -> PathBuf {
        self.root.join("crates/sprites/assets/clawd")
    }

    fn design(&self) -> PathBuf {
        self.root.join("assets/clawd/design")
    }

    fn art(&self) -> PathBuf {
        self.root.join("assets/clawd/src")
    }

    /// Every watched file: activity files, exported sheets, and the art
    /// sources (for `--export`).
    pub(crate) fn stamps(&self) -> Stamps {
        let mut out = Stamps::new();
        let dirs = [
            (self.activities(), &["toml"][..]),
            (self.sheets(), &["json", "png"][..]),
            (self.design(), &["lua"][..]),
            (self.art(), &["aseprite"][..]),
        ];
        for (dir, exts) in dirs {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.filter_map(Result::ok) {
                let p = e.path();
                let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
                if exts.contains(&ext)
                    && let Ok(m) = e.metadata()
                {
                    out.insert(p, (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
                }
            }
        }
        out
    }

    /// Reads the activity files and sheets. Returns every problem at once,
    /// including art that doesn't match its activity.
    pub(crate) fn load(&self) -> Result<(Catalog, SheetCharacter), Vec<String>> {
        let read_dir = |dir: &Path, ext: &str| -> Result<Vec<PathBuf>, Vec<String>> {
            let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
                .map_err(|e| vec![format!("{}: {e}", dir.display())])?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .collect();
            v.sort();
            Ok(v)
        };
        let name = |p: &Path| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let mut texts = Vec::new();
        for p in read_dir(&self.activities(), "toml")? {
            let text =
                std::fs::read_to_string(&p).map_err(|e| vec![format!("{}: {e}", p.display())])?;
            texts.push((name(&p), text));
        }
        let sources: Vec<(&str, &str)> = texts
            .iter()
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .collect();
        let catalog = Catalog::from_sources(&sources)?;
        let mut sheets = Vec::new();
        for json in read_dir(&self.sheets(), "json")? {
            let png = json.with_extension("png");
            let stem = json
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&json)
                .map_err(|e| vec![format!("{}: {e}", json.display())])?;
            let bytes = std::fs::read(&png).map_err(|e| vec![format!("{}: {e}", png.display())])?;
            sheets.push((stem, text, bytes));
        }
        let sources: Vec<(&str, &str, &[u8])> = sheets
            .iter()
            .map(|(n, j, p)| (n.as_str(), j.as_str(), p.as_slice()))
            .collect();
        let character = SheetCharacter::from_sources(&sources)?;
        let problems = character.validate(&catalog);
        if problems.is_empty() {
            Ok((catalog, character))
        } else {
            Err(problems)
        }
    }

    /// The `export_sprites.py` runs for changed art files: a design script
    /// rebuilds its source from the script (a shared module rebuilds all); an
    /// edited source is exported as it is, never rebuilt over. Empty when no
    /// art changed.
    pub(crate) fn export_runs(&self, changed: &[PathBuf]) -> Vec<Vec<String>> {
        let mut build = Vec::new();
        let mut export = Vec::new();
        let mut build_all = false;
        for p in changed {
            let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            let dir = p.parent();
            if dir == Some(self.design().as_path()) {
                if stem.starts_with('_') || stem == "clawd" {
                    build_all = true;
                } else {
                    build.push(stem);
                }
            } else if dir == Some(self.art().as_path()) && !stem.starts_with("_template") {
                export.push(stem);
            }
        }
        let mut runs = Vec::new();
        if build_all {
            runs.push(vec!["--build".to_string()]);
            return runs;
        }
        build.sort();
        build.dedup();
        export.sort();
        export.dedup();
        // Building writes the source and exports it, so those need no export.
        export.retain(|n| !build.contains(n));
        if !build.is_empty() {
            runs.push(
                std::iter::once("--build".to_string())
                    .chain(build)
                    .collect(),
            );
        }
        if !export.is_empty() {
            runs.push(export);
        }
        runs
    }

    fn run_export(&self, args: &[String]) {
        let python = if cfg!(windows) { "python" } else { "python3" };
        eprintln!(
            "exporting sprites: {python} scripts/export_sprites.py {}",
            args.join(" ")
        );
        match std::process::Command::new(python)
            .arg(self.root.join("scripts/export_sprites.py"))
            .args(args)
            .current_dir(&self.root)
            .status()
        {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("export failed ({s})"),
            Err(e) => eprintln!("could not run {python}: {e}"),
        }
    }
}

/// Paths added, removed or changed between two scans.
fn changed(old: &Stamps, new: &Stamps) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = new
        .iter()
        .filter(|(p, s)| old.get(*p) != Some(*s))
        .map(|(p, _)| p.clone())
        .collect();
    out.extend(old.keys().filter(|p| !new.contains_key(*p)).cloned());
    out
}

/// A scene with the activity in it, made with these files.
pub(crate) fn scene(
    catalog: &Catalog,
    character: &SheetCharacter,
    activity: &str,
    seed: u64,
    (w, h): (u32, u32),
) -> Result<ScenePackage, String> {
    if catalog.get(activity).is_none() {
        let ids: Vec<&str> = catalog.activities.iter().map(|a| a.id.as_str()).collect();
        return Err(format!(
            "unknown activity `{activity}`; available: {}",
            ids.join(", ")
        ));
    }
    let ctx = ContentContext {
        enabled_activities: vec![activity.to_string()],
        data_hash: catalog.data_hash,
        ..ContentContext::default()
    };
    let recipe = compose_with(catalog, seed, Aspect::from_size(w, h), &ctx);
    if recipe.activity.as_deref() != Some(activity) {
        return Err(format!("`{activity}` fits no scene for seed {seed}"));
    }
    ScenePackage::capture_with(&recipe, w, h, catalog, character).map_err(|e| e.to_string())
}

// ---- The window ------------------------------------------------------------------

/// How often the files are checked.
const SCAN: Duration = Duration::from_millis(400);
/// Quiet time after a resize before the scene is rebuilt at the new size.
const SETTLE: Duration = Duration::from_millis(300);

struct Preview {
    args: PreviewArgs,
    sources: Sources,
    loaded: Option<(Catalog, SheetCharacter)>,
    stamps: Stamps,
    seed: u64,
    theme: Theme,
    buddy: bool,
    size: (u32, u32),
    frame_every: Duration,
    window: Option<Rc<Window>>,
    context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    player: Option<Player>,
    /// Archetype and theme on screen, for the title.
    showing: String,
    frame: Vec<u32>,
    next_frame: Instant,
    next_scan: Instant,
    rebuild_at: Option<Instant>,
    opened: Instant,
    failed: bool,
}

impl Preview {
    fn reload(&mut self) {
        match self.sources.load() {
            Ok(l) => {
                self.loaded = Some(l);
                self.failed = false;
                tracing::info!(target: "clairlune_cli", "preview: files loaded");
            }
            Err(errors) => {
                self.failed = true;
                eprintln!("the files have problems; showing the last scene that loaded:");
                for e in errors {
                    eprintln!("  {e}");
                }
            }
        }
    }

    fn rebuild(&mut self) {
        self.rebuild_at = None;
        let Some((catalog, character)) = &self.loaded else {
            return;
        };
        let started = Instant::now();
        match scene(
            catalog,
            character,
            &self.args.activity,
            self.seed,
            self.size,
        ) {
            Ok(pkg) => {
                let themes = pkg.themes();
                if !themes.contains(&self.theme) {
                    self.theme = themes[0];
                }
                self.showing = format!("{} · {}", pkg.recipe.archetype, self.theme.as_str());
                self.player = Player::new(Arc::new(pkg), self.theme, self.buddy).map(|mut p| {
                    // As live on a GPU shows it.
                    p.set_smooth(true);
                    p
                });
                self.failed = false;
                eprintln!(
                    "seed {}: {} ({} ms)",
                    self.seed,
                    self.showing,
                    started.elapsed().as_millis()
                );
            }
            Err(e) => {
                self.failed = true;
                eprintln!("seed {}: {e}", self.seed);
            }
        }
        self.update_title();
    }

    fn update_title(&self) {
        if let Some(w) = &self.window {
            let error = if self.failed {
                "  (errors: see the terminal)"
            } else {
                ""
            };
            w.set_title(&format!(
                "{} · seed {} · {}{error}   N/P scene · T theme · B Buddy · R reload · Esc",
                self.args.activity, self.seed, self.showing
            ));
        }
    }

    fn scan(&mut self) {
        let now = self.sources.stamps();
        let changed = changed(&self.stamps, &now);
        self.stamps = now;
        if changed.is_empty() {
            return;
        }
        if self.args.export {
            let runs = self.sources.export_runs(&changed);
            for args in &runs {
                self.sources.run_export(args);
            }
            if !runs.is_empty() {
                // The export's own writes are part of this change.
                self.stamps = self.sources.stamps();
            }
        }
        let watched_by_load = |p: &PathBuf| {
            p.starts_with(self.sources.activities()) || p.starts_with(self.sources.sheets())
        };
        if changed.iter().any(watched_by_load) || self.args.export {
            eprintln!("files changed; reloading");
            self.reload();
            self.rebuild();
        }
    }

    fn draw(&mut self) {
        let (Some(player), Some(surface), Some(window)) =
            (&mut self.player, &mut self.surface, &self.window)
        else {
            return;
        };
        let t = player.now_ms();
        player.advance(t);
        let (pw, ph) = player.size();
        self.frame.resize((pw * ph) as usize, 0);
        player.write_all(&mut self.frame);
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        // The scene at the top left; any margin (mid-resize) in its darkest
        // colour.
        let (sw, sh) = (size.width as usize, size.height as usize);
        let edge = self.frame.first().copied().unwrap_or(0);
        for y in 0..sh {
            let row = &mut buf[y * sw..(y + 1) * sw];
            if y < ph as usize {
                let n = sw.min(pw as usize);
                row[..n].copy_from_slice(&self.frame[y * pw as usize..y * pw as usize + n]);
                row[n..].fill(edge);
            } else {
                row.fill(edge);
            }
        }
        window.pre_present_notify();
        let _ = buf.present();
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        match key {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Named(NamedKey::ArrowRight) => self.step(1),
            Key::Named(NamedKey::ArrowLeft) => self.step(-1),
            Key::Character(c) => match c.to_lowercase().as_str() {
                "n" => self.step(1),
                "p" => self.step(-1),
                "t" => {
                    self.theme = match self.theme {
                        Theme::Dark => Theme::Light,
                        Theme::Light => Theme::Dark,
                    };
                    self.rebuild();
                }
                "b" => {
                    self.buddy = !self.buddy;
                    self.rebuild();
                }
                "r" => {
                    self.reload();
                    self.rebuild();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn step(&mut self, by: i64) {
        self.seed = self.seed.wrapping_add_signed(by);
        self.rebuild();
    }
}

impl ApplicationHandler for Preview {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(format!("{}: preview", self.args.activity))
            .with_inner_size(PhysicalSize::new(self.size.0, self.size.1));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("could not open a window: {e}");
                event_loop.exit();
                return;
            }
        };
        let drawn = softbuffer::Context::new(window.clone()).and_then(|ctx| {
            let surface = softbuffer::Surface::new(&ctx, window.clone())?;
            Ok((ctx, surface))
        });
        match drawn {
            Ok((ctx, surface)) => {
                self.context = Some(ctx);
                self.surface = Some(surface);
            }
            Err(e) => {
                eprintln!("could not draw into the window: {e}");
                event_loop.exit();
                return;
            }
        }
        // The window may open at another size than asked (a small screen).
        let s = window.inner_size();
        if s.width > 0 && s.height > 0 {
            self.size = (s.width, s.height);
        }
        self.window = Some(window);
        self.rebuild();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) if s.width > 0 && s.height > 0 => {
                if (s.width, s.height) != self.size {
                    self.size = (s.width, s.height);
                    self.rebuild_at = Some(Instant::now() + SETTLE);
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => self.key(event_loop, &logical_key),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self
            .args
            .exit_after
            .is_some_and(|s| now.duration_since(self.opened).as_secs_f64() >= s)
        {
            event_loop.exit();
            return;
        }
        if now >= self.next_scan {
            self.scan();
            self.next_scan = now + SCAN;
        }
        if self.rebuild_at.is_some_and(|at| now >= at) {
            self.rebuild();
        }
        if now >= self.next_frame {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            self.next_frame += self.frame_every;
            if self.next_frame < now {
                self.next_frame = now + self.frame_every;
            }
        }
        let mut wake = self.next_frame.min(self.next_scan);
        if let Some(at) = self.rebuild_at {
            wake = wake.min(at);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

pub fn preview(args: PreviewArgs) -> Result<(), String> {
    let sources = Sources::find(args.root.as_deref())?;
    eprintln!("watching {}", sources.root.display());
    let stamps = sources.stamps();
    let loaded = sources
        .load()
        .map_err(|errors| format!("the files have problems:\n  {}", errors.join("\n  ")))?;
    // Fail early, in the terminal, on an unknown activity.
    scene(&loaded.0, &loaded.1, &args.activity, args.seed, (320, 180)).map(|_| ())?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let now = Instant::now();
    let mut app = Preview {
        seed: args.seed,
        theme: args.theme.into(),
        buddy: true,
        size: args.size,
        frame_every: Duration::from_secs_f64(1.0 / f64::from(fps(args.smoothness))),
        args,
        sources,
        loaded: Some(loaded),
        stamps,
        window: None,
        context: None,
        surface: None,
        player: None,
        showing: String::new(),
        frame: Vec::new(),
        next_frame: now,
        next_scan: now + SCAN,
        rebuild_at: None,
        opened: now,
        failed: false,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_scene::activity::builtin_catalog;
    use clairlune_sprites::builtin_character;

    fn repo() -> Sources {
        Sources::find(Some(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))).unwrap()
    }

    #[test]
    fn the_files_on_disk_are_the_builtin_ones() {
        let (catalog, character) = repo().load().unwrap();
        assert_eq!(catalog.data_hash, builtin_catalog().data_hash);
        assert_eq!(character.colors(), builtin_character().colors());
    }

    #[test]
    fn scenes_show_the_activity() {
        let (catalog, character) = repo().load().unwrap();
        for seed in 1..4 {
            let pkg = scene(&catalog, &character, "warm_hands", seed, (320, 180)).unwrap();
            assert_eq!(pkg.recipe.activity.as_deref(), Some("warm_hands"));
        }
        assert!(scene(&catalog, &character, "juggling", 1, (320, 180)).is_err());
    }

    #[test]
    fn edits_are_noticed() {
        let dir = std::env::temp_dir().join(format!("clairlune-preview-{}", std::process::id()));
        let acts = dir.join("crates/scene/activities");
        std::fs::create_dir_all(&acts).unwrap();
        std::fs::create_dir_all(dir.join("crates/sprites/assets/clawd")).unwrap();
        let file = acts.join("a.toml");
        std::fs::write(&file, "id = 1").unwrap();
        let s = Sources::find(Some(&dir)).unwrap();
        let before = s.stamps();
        std::fs::write(&file, "id = 12").unwrap();
        let after = s.stamps();
        assert_eq!(changed(&before, &after), vec![file.clone()]);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(changed(&after, &s.stamps()), vec![file]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn art_changes_pick_the_export() {
        let s = repo();
        let design = |n: &str| s.design().join(format!("{n}.lua"));
        let art = |n: &str| s.art().join(format!("{n}.aseprite"));
        let runs = |v: &[&[&str]]| -> Vec<Vec<String>> {
            v.iter()
                .map(|r| r.iter().map(|a| a.to_string()).collect())
                .collect()
        };
        let sheet = s.sheets().join("warm_hands.json");
        assert!(s.export_runs(std::slice::from_ref(&sheet)).is_empty());
        assert_eq!(
            s.export_runs(&[art("warm_hands")]),
            runs(&[&["warm_hands"]])
        );
        // A design change rebuilds that one; an edited source is only exported.
        assert_eq!(
            s.export_runs(&[design("stroll"), art("warm_hands"), art("stroll")]),
            runs(&[&["--build", "stroll"], &["warm_hands"]])
        );
        assert_eq!(s.export_runs(&[design("clawd")]), runs(&[&["--build"]]));
        assert!(s.export_runs(&[art("_template")]).is_empty());
    }
}
