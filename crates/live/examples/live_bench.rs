//! What live playback costs: plays a scene on every monitor behind the
//! desktop icons for a while and prints frame time, CPU and memory. Never
//! sets the wallpaper and writes no files.
//!
//! `cargo run -p clairlune-live --example live_bench --release -- [seconds] [fps]`

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    use clairlune_live::Player;
    use clairlune_live::desktop::Desktop;
    use clairlune_platform::windows_live::{
        process_cpu_time, process_memory, pump_messages_until, trim_memory,
    };
    use clairlune_platform::{detect, init_dpi_awareness};
    use clairlune_render::ScenePackage;
    use clairlune_scene::{Aspect, ContentContext, Theme, compose};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(12);
    let fps: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(12);
    init_dpi_awareness();
    let monitors = detect().expect("host").monitors().expect("monitors");
    let mut scenes = Vec::new();
    for (i, m) in monitors.iter().enumerate() {
        // A campfire scene on every monitor: the busiest case.
        let ctx = ContentContext {
            enabled_activities: vec!["warm_hands".into()],
            ..ContentContext::default()
        };
        let r = compose(40 + i as u64, Aspect::from_size(m.width, m.height), &ctx);
        let pkg = ScenePackage::capture(&r, m.width, m.height).expect("scene");
        let theme = if pkg.themes().contains(&Theme::Dark) {
            Theme::Dark
        } else {
            pkg.themes()[0]
        };
        scenes.push((
            m.clone(),
            Player::new(Arc::new(pkg), theme, true).expect("player"),
        ));
    }
    trim_memory();
    let before = process_memory();
    let mut desktop = Desktop::start(scenes).expect("live");
    println!(
        "pid {} presenting with {}",
        std::process::id(),
        desktop.presenter()
    );
    let frame = Duration::from_secs_f64(1.0 / f64::from(fps.max(1)));
    let start = Instant::now();
    let cpu0 = process_cpu_time();
    let (mut frames, mut pixels, mut busy, mut peak) = (0u64, 0u64, Duration::ZERO, 0u64);
    let mut next = Instant::now();
    while start.elapsed() < Duration::from_secs(seconds) {
        let s = desktop.tick().expect("frame");
        frames += 1;
        pixels += s.pixels;
        busy += s.busy;
        if frames % 12 == 0 {
            peak = peak.max(process_memory());
        }
        next = (next + frame).max(Instant::now());
        pump_messages_until(next);
    }
    let wall = start.elapsed();
    let cpu = process_cpu_time().saturating_sub(cpu0);
    drop(desktop);
    let mb = |b: u64| b >> 20;
    println!(
        "{frames} frames in {:.1} s; {:.2} ms busy and {:.0} px copied per frame",
        wall.as_secs_f64(),
        busy.as_secs_f64() * 1e3 / frames.max(1) as f64,
        pixels as f64 / frames.max(1) as f64
    );
    println!(
        "CPU {:.2} % of one core; memory {} MB before, {} MB peak",
        cpu.as_secs_f64() / wall.as_secs_f64() * 100.0,
        mb(before),
        mb(peak)
    );
}
