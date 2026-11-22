//! Starting at sign-in, quietly: the entry runs the app with
//! [`BACKGROUND_ARG`], so it comes up in the tray (the menu bar on macOS)
//! with no window. Windows keeps it in the per-user Run key (`windows.rs`);
//! macOS in a per-user launch agent.
// Each system writes one kind of entry; the others' are only tested here.
#![allow(dead_code)]

use std::path::Path;

/// The argument a sign-in launch passes: start quietly, no window.
pub const BACKGROUND_ARG: &str = "--background";

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
#[cfg(target_os = "macos")]
fn entry(exe: &Path) -> Option<(std::path::PathBuf, String)> {
    let path = crate::home_dir()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LABEL}.plist"));
    Some((path, launch_agent(exe)))
}

/// macOS: the entry is there, but turned off in System Settings > General >
/// Login Items (macOS 13 and later let the user switch background items
/// off there; Clairlune can't turn it back on itself).
#[cfg(target_os = "macos")]
pub(crate) fn needs_approval(exe: &Path) -> bool {
    use objc2_foundation::{NSString, NSURL};
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    let Some((path, _)) = entry(exe).filter(|(path, _)| path.exists()) else {
        return false;
    };
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    // SAFETY: a file URL to the agent's plist (macOS 13, the oldest
    // supported, has this call).
    unsafe { SMAppService::statusForLegacyURL(&url) == SMAppServiceStatus::RequiresApproval }
}

/// macOS: System Settings at General > Login Items.
#[cfg(target_os = "macos")]
pub(crate) fn open_login_items() {
    // SAFETY: no arguments; macOS 13 has it.
    unsafe { objc2_service_management::SMAppService::openSystemSettingsLoginItems() };
}

/// Whether the entry is there and starts this `exe`.
#[cfg(target_os = "macos")]
pub(crate) fn registered(exe: &Path) -> bool {
    entry(exe).is_some_and(|(path, text)| std::fs::read_to_string(path).is_ok_and(|t| t == text))
}

/// Writes or removes the entry.
#[cfg(target_os = "macos")]
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
