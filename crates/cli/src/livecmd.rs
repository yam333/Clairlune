//! `live`: plays a scene per monitor behind the desktop icons for a while,
//! then leaves the still (the same scene's hero frame) as the wallpaper.
//! A developer check for the live hosts and their cost (M7).

use clap::Args;

#[derive(Args)]
pub struct LiveArgs {
    /// How long to play, in seconds (up to a week).
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=604_800))]
    seconds: u64,
    /// Frames per second (6, 12, 24, 30 or 60 in the app).
    #[arg(long, default_value_t = 12)]
    fps: u32,
    /// Base seed (random when omitted).
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long, value_enum, default_value_t = crate::ThemeArg::Dark)]
    theme: crate::ThemeArg,
    /// Force an activity on every monitor.
    #[arg(long)]
    activity: Option<String>,
    /// Force a landscape on every monitor.
    #[arg(long)]
    archetype: Option<String>,
    /// Without the character.
    #[arg(long)]
    no_buddy: bool,
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn live(_args: LiveArgs) -> Result<(), String> {
    Err("`live` runs on Windows and Linux; on macOS, try Live in the app".to_string())
}

/// Linux: plays the desktop's scene on our own surfaces for a while; the
/// wallpaper itself is never touched, and shows again when they close.
#[cfg(target_os = "linux")]
pub fn live(args: LiveArgs) -> Result<(), String> {
    use crate::setcmd::monitor_seed;
    use crate::{random_seed, restrict_activity, restrict_archetype};
    use clairlune_live::Player;
    use clairlune_live::linux::LinuxDesktop;
    use clairlune_render::ScenePackage;
    use clairlune_scene::{Aspect, ContentContext, Theme, compose};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    clairlune_live::linux::available()?;
    // The size only: window managers with no wallpaper host (i3, Openbox)
    // play live too.
    let m = clairlune_platform::linux_desktop_monitor();
    let mut ctx = ContentContext::default();
    if let Some(id) = &args.archetype {
        restrict_archetype(&mut ctx, id)?;
    }
    if let Some(id) = &args.activity {
        restrict_activity(&mut ctx, id)?;
    }
    let base = args.seed.unwrap_or_else(random_seed);
    let seed = monitor_seed(base, &m);
    let recipe = compose(seed, Aspect::from_size(m.width, m.height), &ctx);
    let pkg = ScenePackage::capture(&recipe, m.width, m.height)
        .map_err(|e| format!("seed {seed}: {e}"))?;
    let wanted: Theme = args.theme.into();
    let theme = if pkg.themes().contains(&wanted) {
        wanted
    } else {
        pkg.themes()[0]
    };
    println!(
        "{}x{}  seed {seed}  {} in {}  ({})",
        m.width,
        m.height,
        recipe.activity.as_deref().unwrap_or("-"),
        recipe.archetype,
        theme.as_str()
    );
    let player = Player::new(Arc::new(pkg), theme, !args.no_buddy).ok_or("no such theme")?;
    let mut desktop = LinuxDesktop::start(vec![(m, player)])?;
    let fps = args.fps.clamp(1, desktop.max_fps());
    println!(
        "live: playing for {} s at {fps} fps, presenting with {}",
        args.seconds,
        desktop.presenter()
    );
    let frame = Duration::from_secs_f64(1.0 / f64::from(fps));
    let started = Instant::now();
    let end = started + Duration::from_secs(args.seconds);
    let (mut frames, mut pixels, mut busy, mut worst) =
        (0u64, 0u64, Duration::ZERO, Duration::ZERO);
    let mut next = Instant::now();
    while Instant::now() < end {
        let s = desktop.tick()?;
        frames += 1;
        pixels += s.pixels;
        busy += s.busy;
        worst = worst.max(s.busy);
        next += frame;
        let now = Instant::now();
        if next < now {
            next = now; // fell behind: don't try to catch up
        }
        std::thread::sleep(next.min(end).saturating_duration_since(Instant::now()));
    }
    let wall = started.elapsed();
    drop(desktop);
    println!(
        "{frames} frames in {:.1} s ({:.1} fps); per frame {:.2} ms busy (worst {:.2}), {:.0} px copied",
        wall.as_secs_f64(),
        frames as f64 / wall.as_secs_f64(),
        busy.as_secs_f64() * 1e3 / frames.max(1) as f64,
        worst.as_secs_f64() * 1e3,
        pixels as f64 / frames.max(1) as f64
    );
    println!("stopped; your wallpaper was never changed.");
    Ok(())
}

#[cfg(windows)]
pub fn live(args: LiveArgs) -> Result<(), String> {
    use crate::setcmd::monitor_seed;
    use crate::{random_seed, restrict_activity, restrict_archetype};
    use clairlune_live::Player;
    use clairlune_live::desktop::Desktop;
    use clairlune_platform::windows_live::{process_cpu_time, process_memory, pump_messages_until};
    use clairlune_platform::{StillStore, detect, init_dpi_awareness};
    use clairlune_render::{ScenePackage, encode_png};
    use clairlune_scene::{Aspect, ContentContext, Theme, compose};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    init_dpi_awareness();
    let host = detect().map_err(|e| e.to_string())?;
    let monitors = host.monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Err("no monitors found".to_string());
    }
    let store = StillStore::default_location().map_err(|e| format!("still store: {e}"))?;
    let snapshot = host.snapshot().map_err(|e| e.to_string())?;
    if store
        .remember_if_users(&snapshot)
        .map_err(|e| format!("saving the current wallpaper: {e}"))?
    {
        println!("saved your current wallpaper; `restore` puts it back");
    }
    let mut ctx = ContentContext::default();
    if let Some(id) = &args.archetype {
        restrict_archetype(&mut ctx, id)?;
    }
    if let Some(id) = &args.activity {
        restrict_activity(&mut ctx, id)?;
    }
    let base = args.seed.unwrap_or_else(random_seed);
    let wanted: Theme = args.theme.into();
    println!("base seed {base}");
    let mut scenes = Vec::new();
    let mut stills = Vec::new();
    for m in &monitors {
        let seed = monitor_seed(base, m);
        let recipe = compose(seed, Aspect::from_size(m.width, m.height), &ctx);
        let pkg = ScenePackage::capture(&recipe, m.width, m.height)
            .map_err(|e| format!("seed {seed}: {e}"))?;
        let theme = if pkg.themes().contains(&wanted) {
            wanted
        } else {
            pkg.themes()[0]
        };
        // The still under the live window is its first frame.
        let (still, palette) = pkg.still(theme, !args.no_buddy).ok_or("no still")?;
        let mut png = Vec::new();
        encode_png(&still, palette, &mut png).map_err(|e| e.to_string())?;
        let path = store.write(&m.id, &png).map_err(|e| e.to_string())?;
        host.set_static(m, &path).map_err(|e| e.to_string())?;
        store.prune(&m.id, &path);
        println!(
            "{} {}x{}  seed {seed}  {} in {}  ({})",
            if m.primary { "*" } else { " " },
            m.width,
            m.height,
            recipe.activity.as_deref().unwrap_or("-"),
            recipe.archetype,
            theme.as_str()
        );
        let player = Player::new(Arc::new(pkg), theme, !args.no_buddy).ok_or("no such theme")?;
        scenes.push((m.clone(), player));
        stills.push((m.clone(), path));
    }

    let mem_before = process_memory();
    let mut desktop = Desktop::start(scenes)?;
    // The host's own cap (GDI, or a scrolling scene) as the app has it.
    let fps = args.fps.clamp(1, desktop.max_fps());
    println!(
        "live: playing behind the icons for {} s at {fps} fps, presenting with {}",
        args.seconds,
        desktop.presenter()
    );
    let frame = Duration::from_secs_f64(1.0 / f64::from(fps));
    let started = Instant::now();
    let cpu_start = process_cpu_time();
    let end = started + Duration::from_secs(args.seconds);
    let (mut frames, mut pixels, mut busy, mut worst) =
        (0u64, 0u64, Duration::ZERO, Duration::ZERO);
    let (mut rects, mut upload, mut present) = (0usize, Duration::ZERO, Duration::ZERO);
    let mut covered_frames = 0u64;
    let mut stalls = 0usize;
    let mut stall_frames = Vec::new();
    let mut next = Instant::now();
    let mut peak_mem = 0u64;
    while Instant::now() < end {
        if !desktop.healthy() {
            println!("the desktop layer went away (Explorer restarted?); stopping");
            break;
        }
        let s = desktop.tick()?;
        frames += 1;
        pixels += s.pixels;
        busy += s.busy;
        worst = worst.max(s.busy);
        rects += s.rects;
        covered_frames += s.covered as u64;
        stalls += s.stalls;
        if s.stalls > 0 {
            stall_frames.push(frames);
        }
        upload += s.upload;
        present += s.present;
        if frames % 24 == 0 {
            peak_mem = peak_mem.max(process_memory());
        }
        next += frame;
        let now = Instant::now();
        if next < now {
            next = now; // fell behind: don't try to catch up
        }
        pump_messages_until(next.min(end));
    }
    let wall = started.elapsed();
    let cpu = process_cpu_time().saturating_sub(cpu_start);
    peak_mem = peak_mem.max(process_memory());
    drop(desktop);
    // A stale desktop layer can keep showing the last live frame: set the
    // still again so the desktop repaints.
    for (m, path) in &stills {
        host.set_static(m, path).map_err(|e| e.to_string())?;
    }
    pump_messages_until(Instant::now() + Duration::from_millis(200));
    let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
    println!(
        "{frames} frames in {:.1} s ({:.1} fps); per frame {:.2} ms busy (worst {:.2}), {:.0} px copied",
        wall.as_secs_f64(),
        frames as f64 / wall.as_secs_f64(),
        busy.as_secs_f64() * 1e3 / frames.max(1) as f64,
        worst.as_secs_f64() * 1e3,
        pixels as f64 / frames.max(1) as f64
    );
    let per = |d: Duration| d.as_secs_f64() * 1e3 / frames.max(1) as f64;
    if stalls > 0 {
        println!("  presents that waited over 50 ms: {stalls}, at frames {stall_frames:?}");
    }
    if covered_frames > 0 {
        println!("  monitor-frames paused behind a covering window: {covered_frames}");
    }
    println!(
        "  of which upload {:.2} ms ({:.0} rects), draw and present {:.2} ms",
        per(upload),
        rects as f64 / frames.max(1) as f64,
        per(present)
    );
    println!(
        "CPU {:.2}% of one core; memory {:.0} MB before live, {:.0} MB peak",
        cpu.as_secs_f64() / wall.as_secs_f64() * 100.0,
        mb(mem_before),
        mb(peak_mem)
    );
    println!("stopped; the still is the wallpaper. `restore` puts your own back.");
    Ok(())
}
