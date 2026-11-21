//! Starting at sign-in, quietly: the entry runs the app with
//! [`BACKGROUND_ARG`], so it comes up in the tray (the menu bar on macOS)
//! with no window. Windows keeps it in the per-user Run key (`windows.rs`);
//! Linux in an XDG autostart entry; macOS in a per-user launch agent.
// Each system writes one kind of entry; the others' are only tested here.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The argument a sign-in launch passes: start quietly, no window.
pub const BACKGROUND_ARG: &str = "--background";

/// What the entry starts. An AppImage runs from a new mount every time, so
/// the entry names the AppImage file itself.
fn launch_path(exe: &Path) -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Some(p) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    exe.to_path_buf()
}

/// One argument of a desktop entry's `Exec` line, quoted: inside the quotes
/// `"`, `` ` ``, `$` and `\` take a backslash, and the file's own string
/// escaping doubles every backslash after that.
fn exec_arg(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for ch in s.chars() {
        if matches!(ch, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(ch);
    }
    q.push('"');
    q.replace('\\', "\\\\")
}

/// Linux: the autostart entry.
pub(crate) fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Clairlune\nComment=Animated ASCII-art wallpapers\nExec={} {BACKGROUND_ARG}\nIcon=clairlune\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        exec_arg(&exe.to_string_lossy())
    )
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// macOS: the launch agent, run once at each sign-in.
pub(crate) fn launch_agent(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \t<key>Label</key>\n\t<string>{LABEL}</string>\n\
         \t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t\t<string>{BACKGROUND_ARG}</string>\n\t</array>\n\
         \t<key>RunAtLoad</key>\n\t<true/>\n\
         \t<key>ProcessType</key>\n\t<string>Interactive</string>\n\
         </dict>\n</plist>\n",
        xml(&exe.to_string_lossy())
    )
}

/// The launch agent's label (the app's bundle identifier).
const LABEL: &str = "app.clairlune";

/// Where the entry lives, and its text for `exe`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn entry(exe: &Path) -> Option<(PathBuf, String)> {
    let exe = launch_path(exe);
    #[cfg(target_os = "linux")]
    {
        let path = crate::config_dir()?
            .join("autostart")
            .join("clairlune.desktop");
        Some((path, desktop_entry(&exe)))
    }
    #[cfg(target_os = "macos")]
    {
        let path = crate::home_dir()?
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{LABEL}.plist"));
        Some((path, launch_agent(&exe)))
    }
}

/// Whether the entry is there and starts this `exe`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn registered(exe: &Path) -> bool {
    entry(exe).is_some_and(|(path, text)| std::fs::read_to_string(path).is_ok_and(|t| t == text))
}

/// Writes or removes the entry.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn set(exe: &Path, enable: bool) -> Result<(), crate::PlatformError> {
    let os = |e: std::io::Error| crate::PlatformError::Os(e.to_string());
    let (path, text) =
        entry(exe).ok_or_else(|| crate::PlatformError::Os("no home or config folder".into()))?;
    if enable {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(os)?;
        }
        std::fs::write(&path, text).map_err(os)
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(os(e)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_autostart_entry_quotes_the_path_and_starts_quietly() {
        let text = desktop_entry(Path::new("/opt/My Apps/clair$lune"));
        assert!(
            text.contains("Exec=\"/opt/My Apps/clair\\\\$lune\" --background\n"),
            "{text}"
        );
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn the_launch_agent_escapes_the_path_and_starts_quietly() {
        let text = launch_agent(Path::new(
            "/Applications/A & B.app/Contents/MacOS/clairlune",
        ));
        assert!(
            text.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/clairlune</string>")
        );
        assert!(text.contains("<string>--background</string>"));
        assert!(text.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(text.contains("<string>app.clairlune</string>"));
    }
}
