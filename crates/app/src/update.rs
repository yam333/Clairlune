//! Checking for a new version: once a day (when Settings allows it) and on
//! request, the app asks GitHub for its latest release, the way most tray
//! apps do (Sparkle on the Mac checks daily by default). The request goes
//! through the system's own `curl` (on Windows 10 and later, macOS and
//! Linux alike), so the app carries no networking code; it sends nothing
//! but the request itself. A newer version shows in Settings and the tray
//! menu: one click installs it ([`crate::install`]), or opens its release
//! page where Clairlune can't install it itself.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where the releases are.
pub const RELEASES_API: &str = "https://api.github.com/repos/yam333/Clairlune/releases/latest";
/// The release list, for when the newest can't be named.
pub const RELEASES_PAGE: &str = "https://github.com/yam333/Clairlune/releases";
/// Where release downloads are; nothing else is downloaded.
pub const DOWNLOADS: &str = "https://github.com/yam333/Clairlune/releases/download/";
/// How often the automatic check runs.
pub const EVERY_SECS: u64 = 24 * 60 * 60;

/// What the last check found, kept in `<data>/update.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    /// When it last asked (Unix seconds), whatever the answer.
    pub checked: u64,
    /// The newest release's version and page, when one was found.
    pub latest: Option<String>,
    pub url: Option<String>,
}

impl UpdateState {
    pub fn load(data_dir: &Path) -> UpdateState {
        std::fs::read_to_string(data_dir.join("update.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) {
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(data_dir.join("update.json"), text);
        }
    }

    /// The newer version, if the last check found one.
    pub fn newer(&self) -> Option<&str> {
        self.latest
            .as_deref()
            .filter(|v| is_newer(v, env!("CARGO_PKG_VERSION")))
    }

    /// Whether the daily check is due at `now`.
    pub fn due(&self, now: u64) -> bool {
        now.saturating_sub(self.checked) >= EVERY_SECS
    }
}

/// The newest release: its version (without the `v`), page and downloads.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub assets: Vec<Asset>,
}

/// One download of a release.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Its SHA-256 (hex), as GitHub lists it.
    pub sha256: Option<String>,
}

/// `x.y.z` as numbers; anything after a `-` or `+` (a pre-release) is left
/// out, and missing parts are 0.
fn parts(v: &str) -> [u64; 3] {
    let core = v.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or("");
    let mut out = [0u64; 3];
    for (slot, p) in out.iter_mut().zip(core.split('.')) {
        *slot = p.parse().unwrap_or(0);
    }
    out
}

/// Whether `candidate` is a later version than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

/// Reads GitHub's answer for the latest release.
pub fn parse_release(json: &str) -> Result<Release, String> {
    #[derive(Deserialize)]
    struct RawAsset {
        name: String,
        browser_download_url: String,
        #[serde(default)]
        size: u64,
        #[serde(default)]
        digest: Option<String>,
    }
    #[derive(Deserialize)]
    struct Raw {
        tag_name: String,
        html_url: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        prerelease: bool,
        #[serde(default)]
        assets: Vec<RawAsset>,
    }
    let raw: Raw = serde_json::from_str(json).map_err(|e| format!("unexpected answer: {e}"))?;
    if raw.draft || raw.prerelease {
        return Err("the newest release isn't final yet".into());
    }
    let assets = raw
        .assets
        .into_iter()
        // Only Clairlune's own downloads.
        .filter(|a| a.browser_download_url.starts_with(DOWNLOADS))
        .map(|a| Asset {
            sha256: a
                .digest
                .as_deref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
                .map(str::to_string),
            name: a.name,
            url: a.browser_download_url,
            size: a.size,
        })
        .collect();
    Ok(Release {
        version: raw.tag_name.trim_start_matches('v').to_string(),
        url: raw.html_url,
        assets,
    })
}

/// Asks GitHub for the latest release (blocking: run it on a thread).
pub fn fetch() -> Result<Release, String> {
    let agent = format!("Clairlune/{}", env!("CARGO_PKG_VERSION"));
    let mut cmd = std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    cmd.args([
        "--silent",
        "--show-error",
        "--fail",
        "--location",
        "--max-time",
        "20",
        "--header",
        "Accept: application/vnd.github+json",
        "--user-agent",
        &agent,
        RELEASES_API,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "curl isn't installed".to_string()
        } else {
            e.to_string()
        }
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim().trim_start_matches("curl: ");
        return Err(if err.is_empty() {
            "no answer from GitHub".into()
        } else {
            err.chars().take(120).collect()
        });
    }
    parse_release(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.15.0", "0.14.1"));
        assert!(is_newer("v0.14.10", "0.14.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.14.1", "0.14.1"));
        assert!(!is_newer("0.14.0", "0.14.1"));
        // A pre-release counts as its version.
        assert!(!is_newer("0.14.1-rc1", "0.14.1"));
    }

    #[test]
    fn the_answer_is_read_and_drafts_are_refused() {
        let ok = r#"{"tag_name":"v0.15.0","html_url":"https://github.com/yam333/Clairlune/releases/tag/v0.15.0","draft":false,"prerelease":false,"assets":[
            {"name":"clairlune_0.15.0_x64-setup.exe","size":6957316,"digest":"sha256:e56c8d94f7c72e6d252a56f590e01628b87d11771546b1e118ded1fc993c8c7a","browser_download_url":"https://github.com/yam333/Clairlune/releases/download/v0.15.0/clairlune_0.15.0_x64-setup.exe"},
            {"name":"elsewhere.exe","size":1,"browser_download_url":"https://example.com/elsewhere.exe"}]}"#;
        assert_eq!(
            parse_release(ok).unwrap(),
            Release {
                version: "0.15.0".into(),
                url: "https://github.com/yam333/Clairlune/releases/tag/v0.15.0".into(),
                assets: vec![Asset {
                    name: "clairlune_0.15.0_x64-setup.exe".into(),
                    url: "https://github.com/yam333/Clairlune/releases/download/v0.15.0/clairlune_0.15.0_x64-setup.exe".into(),
                    size: 6957316,
                    sha256: Some("e56c8d94f7c72e6d252a56f590e01628b87d11771546b1e118ded1fc993c8c7a".into()),
                }],
            }
        );
        assert!(parse_release(r#"{"tag_name":"v1","html_url":"x","prerelease":true}"#).is_err());
        assert!(parse_release("not json").is_err());
    }

    /// Asks GitHub for real: `cargo test -p clairlune update -- --ignored`.
    #[test]
    #[ignore]
    fn github_answers() {
        let r = fetch().unwrap();
        assert!(r.url.contains("/releases/tag/"), "{r:?}");
        eprintln!("latest: {r:?}");
    }

    #[test]
    fn due_once_a_day_and_newer_only_when_newer() {
        let mut s = UpdateState::default();
        assert!(s.due(100_000));
        s.checked = 100_000;
        assert!(!s.due(100_000 + EVERY_SECS - 1));
        assert!(s.due(100_000 + EVERY_SECS));
        s.latest = Some("0.0.1".into());
        assert_eq!(s.newer(), None);
        s.latest = Some("999.0.0".into());
        assert_eq!(s.newer(), Some("999.0.0"));
    }
}
