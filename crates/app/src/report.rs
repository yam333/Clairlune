//! "Report a problem": the details a bug report needs, as text the user
//! reads in Settings first, and a link that opens a new GitHub issue with
//! them filled in. Nothing is sent from here: the user reads the issue on
//! GitHub and sends it, or doesn't.

use std::fmt::Write as _;
use std::sync::OnceLock;

/// Where new issues are opened.
pub const NEW_ISSUE: &str = "https://github.com/yam333/Clairlune/issues/new";
/// Browsers and GitHub take links up to about 8 KB; the log is cut to fit.
const MAX_URL: usize = 7_500;
/// The most log lines a report carries.
pub const LOG_LINES: usize = 30;

/// What the app knows that helps with a problem.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// How it was installed ("the Windows installer").
    pub install: String,
    /// The monitors, by name and size.
    pub monitors: Vec<String>,
    /// "Live at 12 fps, one scene each", "Still frame".
    pub wallpaper: String,
    /// What live playback says (playing, or why not).
    pub live: Option<String>,
    /// The scene on screen ("stroll in cave, seed 42").
    pub showing: String,
    pub crash_reports: usize,
    /// The last lines logged (home folder already written as `~`).
    pub log: Vec<String>,
}

/// The system's name and version, asked once ("Windows 10.0.26300",
/// "macOS 15.1", "Ubuntu 24.04.1 LTS").
pub fn system() -> &'static str {
    static SYSTEM: OnceLock<String> = OnceLock::new();
    SYSTEM.get_or_init(|| {
        let arch = std::env::consts::ARCH;
        let name = ask_system().unwrap_or_else(|| std::env::consts::OS.to_string());
        format!("{name} ({arch})")
    })
}

fn ask_system() -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // "Microsoft Windows [Version 10.0.26300.1234]"
        let out = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let v = text.split("Version ").nth(1)?.trim().trim_end_matches(']');
        Some(format!("Windows {v}"))
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        Some(format!(
            "macOS {}",
            String::from_utf8_lossy(&out.stdout).trim()
        ))
    }
}

/// The details, as Markdown, with at most `log_lines` of the log.
pub fn details(f: &Facts, log_lines: usize) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "- Clairlune {} (scenes v{}), installed from {}",
        env!("CARGO_PKG_VERSION"),
        clairlune_scene::GENERATOR_VERSION,
        f.install
    );
    let _ = writeln!(s, "- System: {}", system());
    if !f.monitors.is_empty() {
        let _ = writeln!(s, "- Monitors: {}", f.monitors.join("; "));
    }
    let _ = writeln!(s, "- Wallpaper: {}", f.wallpaper);
    if let Some(live) = &f.live {
        let _ = writeln!(s, "- Live: {live}");
    }
    if !f.showing.is_empty() {
        let _ = writeln!(s, "- On screen: {}", f.showing);
    }
    let _ = writeln!(s, "- Crash reports: {}", f.crash_reports);
    let start = f.log.len().saturating_sub(log_lines);
    let lines = &f.log[start..];
    if !lines.is_empty() {
        let _ = writeln!(
            s,
            "\n<details><summary>Last {} log lines</summary>\n\n```",
            lines.len()
        );
        for l in lines {
            // Long lines cut short; no stray fences.
            let l: String = l.replace("```", "'''").chars().take(220).collect();
            let _ = writeln!(s, "{l}");
        }
        let _ = writeln!(s, "```\n</details>");
    }
    s
}

/// The issue's text: a space for what happened, then the details.
pub fn body(f: &Facts, log_lines: usize) -> String {
    format!(
        "**What happened?**\n\n<!-- What you saw, and what you expected. A screenshot helps. -->\n\n\n\
         **Details** (filled in by Clairlune)\n\n{}",
        details(f, log_lines)
    )
}

/// The link that opens a new issue with the details filled in (the log cut
/// shorter until the link fits).
pub fn issue_url(f: &Facts) -> String {
    let mut n = LOG_LINES;
    loop {
        let url = format!(
            "{NEW_ISSUE}?labels=bug&title={}&body={}",
            encode("Problem: "),
            encode(&body(f, n))
        );
        if url.len() <= MAX_URL || n == 0 {
            return url;
        }
        n = n.saturating_sub(5);
    }
}

/// Percent-encoding for a query value (everything but the unreserved
/// characters).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b))
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(lines: usize) -> Facts {
        Facts {
            install: "the Windows installer".into(),
            monitors: vec!["Left (2560\u{d7}1440, main)".into()],
            wallpaper: "Live at 12 fps, one scene each".into(),
            live: Some("Playing on 1 monitor at 12 fps.".into()),
            showing: "stroll in cave, seed 42".into(),
            crash_reports: 0,
            log: (0..lines)
                .map(|i| {
                    format!(
                        "2026-11-22T10:00:{i:02}Z INFO clairlune: line {i} {}",
                        "x".repeat(150)
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_details_say_what_matters() {
        let d = details(&facts(3), LOG_LINES);
        assert!(d.contains(env!("CARGO_PKG_VERSION")), "{d}");
        assert!(d.contains("installed from the Windows installer"), "{d}");
        assert!(d.contains("Monitors: Left (2560\u{d7}1440, main)"), "{d}");
        assert!(
            d.contains("Last 3 log lines") && d.contains("line 2"),
            "{d}"
        );
    }

    #[test]
    fn the_link_fits_by_cutting_the_log() {
        let url = issue_url(&facts(200));
        assert!(url.len() <= MAX_URL, "{}", url.len());
        assert!(url.starts_with(
            "https://github.com/yam333/Clairlune/issues/new?labels=bug&title=Problem%3A%20&body="
        ));
        // The newest lines stay.
        assert!(url.contains("line%20199"));
        assert!(!url.contains("line%2010%20"));
    }

    #[test]
    fn values_are_percent_encoded() {
        assert_eq!(encode("a b&c=d\u{d7}"), "a%20b%26c%3Dd%C3%97");
    }
}
