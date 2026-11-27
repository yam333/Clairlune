//! Diagnostics for bug reports (docs/dev/logging.md): the log's last lines in
//! memory, crash reports, log retention, "Detailed logging" for a day, and
//! the diagnostic bundle. Nothing leaves the machine unless the user sends
//! the bundle; paths inside the home folder are written as `~`.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::{EnvFilter, Registry, reload};

/// How many log lines a crash report carries.
const TAIL: usize = 200;
/// The logs folder's size cap (and seven days, by file count).
pub const LOG_CAP_BYTES: u64 = 50 * 1024 * 1024;
/// How long "Detailed logging" lasts.
pub const DETAILED_SECS: u64 = 24 * 60 * 60;

static LINES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();
/// The folder of the wallpaper on screen, for crash reports.
static CURRENT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Keeps the last lines logged, for crash reports.
pub struct Tail;

struct Line(String);

impl Visit for Line {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, " {value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            let _ = write!(self.0, " {value}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Tail {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let m = event.metadata();
        let mut line = Line(format!(
            "{} {} {}:",
            jiff::Timestamp::now(),
            m.level(),
            m.target()
        ));
        event.record(&mut line);
        if let Ok(mut lines) = LINES.lock() {
            if lines.len() == TAIL {
                lines.pop_front();
            }
            lines.push_back(line.0);
        }
    }
}

/// The level filter, swappable at run time; `debug` while detailed.
pub fn filter_layer(detailed: bool) -> reload::Layer<EnvFilter, Registry> {
    let (layer, handle) = reload::Layer::new(filter(detailed));
    let _ = FILTER.set(handle);
    layer
}

fn filter(detailed: bool) -> EnvFilter {
    EnvFilter::try_from_env("CLAIRLUNE_LOG").unwrap_or_else(|_| {
        EnvFilter::new(if detailed {
            "info,clairlune=debug"
        } else {
            "info"
        })
    })
}

/// Switches detailed logging on or off now.
pub fn set_detailed(on: bool) {
    if let Some(h) = FILTER.get() {
        let _ = h.reload(filter(on));
    }
}

/// Remembers the wallpaper on screen, for crash reports.
pub fn set_current(dir: Option<PathBuf>) {
    if let Ok(mut c) = CURRENT.lock() {
        *c = dir;
    }
}

/// Writes `<data>/crashes/<time>.json` when the app panics: version, system,
/// the panic, the last log lines and the recipes on screen.
pub fn install_crash_reports(data_dir: &Path) {
    let dir = data_dir.join("crashes");
    let home = home();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let recipes = CURRENT
            .lock()
            .ok()
            .and_then(|c| c.clone())
            .and_then(|d| std::fs::read_to_string(d.join("recipe.json")).ok())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let lines: Vec<String> = LINES
            .lock()
            .map(|l| l.iter().map(|s| tilde(s, home.as_deref())).collect())
            .unwrap_or_default();
        let report = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "generator_version": clairlune_scene::GENERATOR_VERSION,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "panic": tilde(&info.to_string(), home.as_deref()),
            "backtrace": tilde(&std::backtrace::Backtrace::force_capture().to_string(), home.as_deref()),
            "recipes": recipes,
            "log": lines,
        });
        let _ = std::fs::create_dir_all(&dir);
        let name = jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H-%M-%SZ")
            .to_string();
        if let Ok(text) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(dir.join(format!("{name}.json")), text);
        }
        tracing::error!(target: "clairlune", panic = %info, "panic; crash report written");
        previous(info);
    }));
}

/// A crash report newer than the last one mentioned, once: the next launch
/// says Clairlune stopped unexpectedly.
pub fn new_crash_report(data_dir: &Path) -> Option<PathBuf> {
    let newest = crash_reports(data_dir).pop()?;
    let seen_file = data_dir.join("crashes").join(".seen");
    let name = newest.file_name()?.to_string_lossy().into_owned();
    if std::fs::read_to_string(&seen_file).is_ok_and(|s| s.trim() == name) {
        return None;
    }
    let _ = std::fs::write(&seen_file, &name);
    Some(newest)
}

/// Crash reports, newest last.
pub fn crash_reports(data_dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(data_dir.join("crashes"))
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Deletes the oldest log files while the folder is over `cap` bytes (the
/// newest file, being written, always stays).
pub fn prune_logs(dir: &Path, cap: u64) -> usize {
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok().filter(|m| m.is_file())?;
                    Some((m.modified().ok()?, m.len(), e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    let mut removed = 0;
    for (_, len, path) in files.iter().take(files.len().saturating_sub(1)) {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total -= len;
            removed += 1;
        }
    }
    removed
}

/// The last `n` lines logged, with the home folder written as `~`.
pub fn recent_lines(n: usize) -> Vec<String> {
    let home = home();
    LINES
        .lock()
        .map(|lines| {
            let skip = lines.len().saturating_sub(n);
            lines
                .iter()
                .skip(skip)
                .map(|l| tilde(l, home.as_deref()))
                .collect()
        })
        .unwrap_or_default()
}

/// The home folder, for shortening paths to `~`.
pub fn home() -> Option<String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .filter(|h| h.len() > 3)
}

/// `text` with the home folder written as `~` (either slash style).
pub fn tilde(text: &str, home: Option<&str>) -> String {
    let Some(home) = home else {
        return text.to_string();
    };
    let mut out = text.replace(home, "~");
    out = out.replace(&home.replace('\\', "/"), "~");
    out = out.replace(&home.replace('\\', "\\\\"), "~");
    out
}

/// What the app knows about the system for a bundle.
pub struct SystemInfo {
    pub monitors: Vec<String>,
    pub live: Option<String>,
}

/// Writes the diagnostic bundle zip into `dest` and returns its path:
/// system facts, the last 3 days of logs, crash reports, the recipes on
/// screen and of the last 5 archive entries, and the settings.
pub fn export_bundle(
    data_dir: &Path,
    config_dir: &Path,
    current: Option<&Path>,
    info: &SystemInfo,
    dest: &Path,
) -> Result<PathBuf, String> {
    use std::io::Write;
    let home = home();
    let t = |s: &str| tilde(s, home.as_deref());
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let stamp = jiff::Timestamp::now()
        .strftime("%Y-%m-%dT%H-%M-%SZ")
        .to_string();
    let path = dest.join(format!("clairlune-diagnostics-{stamp}.zip"));
    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut add = |name: &str, text: &str| -> Result<(), String> {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(text.as_bytes()).map_err(|e| e.to_string())
    };
    let mut about = String::new();
    let _ = writeln!(about, "Clairlune {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(about, "generator {}", clairlune_scene::GENERATOR_VERSION);
    let _ = writeln!(
        about,
        "os {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(about, "live: {}", info.live.as_deref().unwrap_or("off"));
    for m in &info.monitors {
        let _ = writeln!(about, "monitor {m}");
    }
    add("about.txt", &about)?;
    // Logs from the last 3 days.
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(3 * 86_400))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(dir) = std::fs::read_dir(data_dir.join("logs")) {
        for e in dir.flatten() {
            let recent = e
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|m| m >= cutoff);
            if recent && let Ok(text) = std::fs::read_to_string(e.path()) {
                add(
                    &format!("logs/{}", e.file_name().to_string_lossy()),
                    &t(&text),
                )?;
            }
        }
    }
    for c in crash_reports(data_dir) {
        if let Ok(text) = std::fs::read_to_string(&c) {
            let name = c
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            add(&format!("crashes/{name}"), &t(&text))?;
        }
    }
    if let Some(dir) = current
        && let Ok(text) = std::fs::read_to_string(dir.join("recipe.json"))
    {
        add("recipes/current.json", &text)?;
    }
    if let Ok(archive) = crate::archive::Archive::open(data_dir.join("archive")) {
        for e in archive.entries().iter().rev().take(5) {
            if let Ok(text) =
                std::fs::read_to_string(archive.dir().join(&e.name).join("recipe.json"))
            {
                add(&format!("recipes/{}.json", e.name), &text)?;
            }
        }
    }
    for name in ["settings.toml", "timeline.toml"] {
        if let Ok(text) = std::fs::read_to_string(config_dir.join(name)) {
            add(name, &t(&text))?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("clairlune-diag-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_panic_writes_a_report_that_the_next_launch_mentions_once() {
        let d = temp("crash");
        set_current(None);
        install_crash_reports(&d);
        let r = std::panic::catch_unwind(|| panic!("boom for the crash report test"));
        assert!(r.is_err());
        let reports = crash_reports(&d);
        assert_eq!(reports.len(), 1);
        let text = std::fs::read_to_string(&reports[0]).unwrap();
        assert!(
            text.contains("boom for the crash report test") && text.contains("\"version\""),
            "{text}"
        );
        assert_eq!(new_crash_report(&d), Some(reports[0].clone()));
        assert_eq!(new_crash_report(&d), None);
        let _ = std::panic::take_hook();
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn home_paths_become_tilde() {
        let home = Some(r"C:\Users\someone");
        assert_eq!(
            tilde(r"data=C:\Users\someone\AppData\Local\clairlune", home),
            r"data=~\AppData\Local\clairlune"
        );
        assert_eq!(tilde("C:/Users/someone/x", home), "~/x");
        assert_eq!(tilde(r#""C:\\Users\\someone\\x""#, home), r#""~\\x""#);
        assert_eq!(tilde("nothing here", home), "nothing here");
    }

    #[test]
    fn logs_are_pruned_oldest_first_under_the_cap() {
        let d = temp("prune");
        for (i, size) in [400usize, 300, 200].iter().enumerate() {
            std::fs::write(
                d.join(format!("clairlune.2026-11-1{i}.log")),
                vec![b'x'; *size],
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(prune_logs(&d, 600), 1);
        assert!(!d.join("clairlune.2026-11-10.log").exists());
        // The newest file stays even alone over the cap.
        assert_eq!(prune_logs(&d, 10), 1);
        assert!(d.join("clairlune.2026-11-12.log").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_bundle_holds_logs_settings_and_recipes_without_home_paths() {
        let data = temp("bundle-data");
        let config = temp("bundle-config");
        let out = temp("bundle-out");
        std::fs::create_dir_all(data.join("logs")).unwrap();
        let home = home().unwrap_or_else(|| "/home/x".into());
        std::fs::write(
            data.join("logs/clairlune.log"),
            format!("started data={home}/stuff\n"),
        )
        .unwrap();
        std::fs::write(config.join("settings.toml"), "[export]\nfolder = \"\"\n").unwrap();
        let info = SystemInfo {
            monitors: vec!["2560x1440 at (0, 0)".into()],
            live: None,
        };
        let zip_path = export_bundle(&data, &config, None, &info, &out).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = zip
            .file_names()
            .map(|n| n.expect("name").into_owned())
            .collect();
        assert!(names.contains(&"about.txt".to_string()), "{names:?}");
        assert!(
            names.contains(&"logs/clairlune.log".to_string()),
            "{names:?}"
        );
        assert!(names.contains(&"settings.toml".to_string()), "{names:?}");
        let mut log = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("logs/clairlune.log").unwrap(), &mut log)
            .unwrap();
        assert!(!log.contains(&home) && log.contains("~/stuff"), "{log}");
        for d in [data, config, out] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }
}
