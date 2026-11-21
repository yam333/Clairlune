//! Installing a newer version in place, the one-click way: the download
//! for this kind of install (the Windows setup.exe, the Mac DMG, the
//! AppImage) is fetched from the GitHub release with the system's `curl`,
//! checked against the SHA-256 GitHub lists for it, and readied; then a
//! small helper script waits for Clairlune to quit, puts the new version in
//! place and starts it again. Installs that something else manages (Scoop,
//! cargo, the .deb) say how to update instead.
//!
//! Removing Clairlune works the same way: a helper waits for it to quit,
//! deletes its folders and, where Clairlune can, the app itself.

use crate::update::Asset;
use std::path::{Path, PathBuf};

/// How this copy of Clairlune was installed.
#[derive(Clone, Debug, PartialEq)]
pub enum Install {
    /// Windows: by setup.exe (for this account), at this exe.
    WindowsSetup { exe: PathBuf },
    /// macOS: this app bundle, in a folder Clairlune can write to.
    MacApp { bundle: PathBuf },
    /// Linux: this AppImage file.
    AppImage { file: PathBuf },
    /// Something else: what it is, and how to update it.
    Manual { name: String, how: String },
}

impl Install {
    /// How it was installed, in a few words (for problem reports).
    pub fn name(&self) -> String {
        match self {
            Install::WindowsSetup { .. } => "the Windows installer".into(),
            Install::MacApp { .. } => "the app (DMG or Homebrew)".into(),
            Install::AppImage { .. } => "an AppImage".into(),
            Install::Manual { name, .. } => name.clone(),
        }
    }

    /// Whether Clairlune can install a new version itself.
    pub fn in_place(&self) -> bool {
        !matches!(self, Install::Manual { .. })
    }

    /// What to do by hand, where Clairlune can't.
    pub fn how(&self) -> Option<&str> {
        match self {
            Install::Manual { how, .. } => Some(how),
            _ => None,
        }
    }

    /// What "Remove Clairlune" does with the app itself: `Ok` names what it
    /// removes, `Err` says how to remove it by hand.
    pub fn removal(&self) -> Result<&'static str, String> {
        match self {
            Install::WindowsSetup { .. } => Ok("the app (its uninstaller runs)"),
            Install::MacApp { .. } => Ok("the app (moved to the Trash)"),
            Install::AppImage { .. } => Ok("the AppImage file"),
            Install::Manual { name, .. } => Err(match name.as_str() {
                "Scoop" => "Then remove the app with: scoop uninstall clairlune".into(),
                "cargo install" => "Then remove the app with: cargo uninstall clairlune".into(),
                "the .deb" => "Then remove the app with: sudo apt remove clairlune".into(),
                _ => "Then delete the app where you keep it.".into(),
            }),
        }
    }

    /// The release download for this install of `version`.
    pub fn asset_name(&self, version: &str) -> Option<String> {
        match self {
            Install::WindowsSetup { .. } => Some(format!("clairlune_{version}_x64-setup.exe")),
            Install::MacApp { .. } => {
                let arch = if cfg!(target_arch = "aarch64") {
                    "aarch64"
                } else {
                    "x64"
                };
                Some(format!("Clairlune_{version}_{arch}.dmg"))
            }
            Install::AppImage { .. } => Some(format!("clairlune_{version}_x86_64.AppImage")),
            Install::Manual { .. } => None,
        }
    }
}

/// This copy's install.
pub fn detect() -> Install {
    let Ok(exe) = std::env::current_exe() else {
        return manual_download("an unknown kind of install");
    };
    let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
    classify(
        &exe,
        std::env::consts::OS,
        appimage.as_deref(),
        &|p| p.exists(),
        &writable,
    )
}

fn manual_download(name: &str) -> Install {
    Install::Manual {
        name: name.into(),
        how: "Download the new version from its release page.".into(),
    }
}

/// [`detect`], from the facts.
fn classify(
    exe: &Path,
    os: &str,
    appimage: Option<&Path>,
    exists: &dyn Fn(&Path) -> bool,
    writable: &dyn Fn(&Path) -> bool,
) -> Install {
    let text = exe.to_string_lossy().replace('\\', "/").to_lowercase();
    if text.contains("/.cargo/bin/") {
        return Install::Manual {
            name: "cargo install".into(),
            how: "Update it with: cargo install clairlune".into(),
        };
    }
    match os {
        "windows" => {
            if text.contains("/scoop/apps/") {
                Install::Manual {
                    name: "Scoop".into(),
                    how: "Update it with: scoop update clairlune".into(),
                }
            } else if exe
                .parent()
                .is_some_and(|dir| exists(&dir.join("uninstall.exe")))
            {
                Install::WindowsSetup {
                    exe: exe.to_path_buf(),
                }
            } else {
                manual_download("a portable copy")
            }
        }
        "macos" => match exe
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
        {
            Some(bundle) if bundle.parent().is_some_and(writable) => Install::MacApp {
                bundle: bundle.to_path_buf(),
            },
            Some(_) => manual_download("the app, in a folder Clairlune can't write to"),
            None => manual_download("a copy outside an app bundle"),
        },
        _ => match appimage {
            Some(file) if file.parent().is_some_and(writable) => Install::AppImage {
                file: file.to_path_buf(),
            },
            Some(_) => manual_download("an AppImage in a folder Clairlune can't write to"),
            None if text.starts_with("/usr/") => Install::Manual {
                name: "the .deb".into(),
                how: "Download the new .deb from its release page, or update it with your package manager.".into(),
            },
            None => manual_download("a copy of its own"),
        },
    }
}

/// Whether Clairlune may write in `dir` (tried, not guessed).
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".clairlune-write-{}", std::process::id()));
    let ok = std::fs::create_dir(&probe).is_ok();
    let _ = std::fs::remove_dir(&probe);
    ok
}

/// A new version, downloaded, checked and ready to go in place.
#[derive(Debug)]
pub struct Prepared {
    pub install: Install,
    pub version: String,
    /// The download (setup.exe, DMG or AppImage).
    pub file: PathBuf,
    /// macOS: the new app, copied out of the DMG next to the old one.
    pub staged: Option<PathBuf>,
}

/// Where downloads go while they're readied.
fn work_dir() -> PathBuf {
    std::env::temp_dir().join("clairlune-update")
}

/// Downloads `asset` for `install`, checks it and readies it. `progress`
/// hears the share done, 0 to 100.
pub fn prepare(
    install: &Install,
    asset: &Asset,
    version: &str,
    progress: &dyn Fn(u32),
) -> Result<Prepared, String> {
    let sha256 = asset
        .sha256
        .as_deref()
        .ok_or("the release lists no checksum for the download")?;
    // An AppImage is downloaded next to the old one, so it can be swapped
    // in by a rename; the rest go to a folder of their own.
    let file = match install {
        Install::AppImage { file } => file.with_file_name(".clairlune-update.AppImage"),
        Install::Manual { .. } => return Err("this install updates another way".into()),
        _ => {
            let dir = work_dir();
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            dir.join(&asset.name)
        }
    };
    download(&asset.url, asset.size, &file, progress)?;
    let got = sha256_file(&file).map_err(|e| e.to_string())?;
    if !got.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(&file);
        return Err("the download doesn't match its checksum".into());
    }
    let staged = match install {
        Install::MacApp { bundle } => Some(stage_mac_app(&file, bundle, version)?),
        Install::AppImage { .. } => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            None
        }
        _ => None,
    };
    Ok(Prepared {
        install: install.clone(),
        version: version.to_string(),
        file,
        staged,
    })
}

/// Downloads `url` (`size` bytes) to `dest`, telling `progress` as it goes.
fn download(url: &str, size: u64, dest: &Path, progress: &dyn Fn(u32)) -> Result<(), String> {
    let agent = format!("Clairlune/{}", env!("CARGO_PKG_VERSION"));
    let mut cmd = std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    cmd.args([
        "--silent",
        "--show-error",
        "--fail",
        "--location",
        "--max-time",
        "900",
        "--user-agent",
        &agent,
        "--output",
    ])
    .arg(dest)
    .arg(url)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "curl isn't installed".to_string()
        } else {
            e.to_string()
        }
    })?;
    let mut last = u32::MAX;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        let have = std::fs::metadata(dest).map_or(0, |m| m.len());
        let share = (have.saturating_mul(100) / size.max(1)).min(100) as u32;
        if share != last {
            progress(share);
            last = share;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut err);
        }
        let err = err.trim().trim_start_matches("curl: ");
        return Err(if err.is_empty() {
            "the download failed".into()
        } else {
            err.chars().take(120).collect()
        });
    }
    let have = std::fs::metadata(dest).map_or(0, |m| m.len());
    if have != size {
        return Err(format!("the download is {have} bytes, not {size}"));
    }
    progress(100);
    Ok(())
}

/// The file's SHA-256, as hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// macOS: copies the app out of the DMG to a hidden folder next to the old
/// one (the same disk, so the swap is a rename) and checks it: its seal
/// holds and it is the version wanted.
fn stage_mac_app(dmg: &Path, bundle: &Path, version: &str) -> Result<PathBuf, String> {
    let run = |cmd: &mut std::process::Command| -> Result<String, String> {
        let out = cmd.output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&out.stderr)
                .trim()
                .chars()
                .take(160)
                .collect())
        }
    };
    let parent = bundle.parent().ok_or("the app has no folder")?;
    let staged = parent.join(".Clairlune-update.app");
    let _ = std::fs::remove_dir_all(&staged);
    let mount = work_dir().join("mount");
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    run(std::process::Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(dmg))
    .map_err(|e| format!("couldn't open the DMG: {e}"))?;
    let copied = run(std::process::Command::new("ditto")
        .arg(mount.join("Clairlune.app"))
        .arg(&staged));
    let _ = run(std::process::Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount));
    copied.map_err(|e| format!("couldn't copy the new app: {e}"))?;
    let checked = run(std::process::Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&staged))
    .map_err(|e| format!("the new app's seal doesn't hold: {e}"))
    .and_then(|_| {
        run(std::process::Command::new("defaults")
            .arg("read")
            .arg(staged.join("Contents").join("Info.plist"))
            .arg("CFBundleShortVersionString"))
    })
    .and_then(|v| {
        if v == version {
            Ok(())
        } else {
            Err(format!("the DMG holds version {v}, not {version}"))
        }
    });
    if let Err(e) = checked {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(e);
    }
    Ok(staged)
}

/// Starts the helper that, once this process (`pid`) is gone, puts the new
/// version in place and starts it with `relaunch` (or doesn't start it,
/// `None`). The caller quits right after.
pub fn finish(p: &Prepared, pid: u32, relaunch: Option<&[&str]>) -> Result<(), String> {
    let mut cmd = match &p.install {
        Install::WindowsSetup { exe } => {
            let script = windows_script(pid, &p.file, exe, relaunch);
            let mut c = std::process::Command::new("powershell.exe");
            c.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &script,
            ]);
            c
        }
        Install::MacApp { bundle } => {
            let staged = p.staged.as_ref().ok_or("the new app isn't ready")?;
            let mut c = std::process::Command::new("/bin/sh");
            c.arg("-c").arg(unix_swap_script(
                pid, staged, bundle, &p.file, relaunch, true,
            ));
            c
        }
        Install::AppImage { file } => {
            let mut c = std::process::Command::new("/bin/sh");
            c.arg("-c").arg(unix_swap_script(
                pid, &p.file, file, &p.file, relaunch, false,
            ));
            c
        }
        Install::Manual { .. } => return Err("this install updates another way".into()),
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    no_window(&mut cmd);
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Starts the helper that, once this process (`pid`) is gone, deletes
/// Clairlune's folders (`dirs`; only folders named `clairlune`, as a
/// guard) and removes the app itself where it can. The caller quits right
/// after.
pub fn remove_after_exit(install: &Install, pid: u32, dirs: &[PathBuf]) -> Result<(), String> {
    let dirs: Vec<&PathBuf> = dirs.iter().filter(|d| is_clairlune_dir(d)).collect();
    let mut cmd = if cfg!(windows) {
        let mut s = format!(
            "$ErrorActionPreference = 'SilentlyContinue'; Wait-Process -Id {pid} -Timeout 120; "
        );
        if let Install::WindowsSetup { exe } = install
            && let Some(dir) = exe.parent()
        {
            // `_?=` runs it where it is (not from a copy that returns at
            // once), so the folder goes after it's done.
            let uninstaller = ps_quote(&dir.join("uninstall.exe").to_string_lossy());
            let arg = ps_quote(&format!("_?={}", dir.to_string_lossy()));
            s.push_str(&format!(
                "Start-Process -FilePath {uninstaller} -ArgumentList '/S',{arg} -Wait; Remove-Item -LiteralPath {} -Recurse -Force; ",
                ps_quote(&dir.to_string_lossy())
            ));
        }
        for d in &dirs {
            s.push_str(&format!(
                "Remove-Item -LiteralPath {} -Recurse -Force; ",
                ps_quote(&d.to_string_lossy())
            ));
        }
        let mut c = std::process::Command::new("powershell.exe");
        c.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &s,
        ]);
        c
    } else {
        let mut s = format!(
            "i=0; while kill -0 {pid} 2>/dev/null && [ $i -lt 600 ]; do sleep 0.2; i=$((i+1)); done; "
        );
        for d in &dirs {
            s.push_str(&format!("rm -rf {}; ", sh_quote(&d.to_string_lossy())));
        }
        match install {
            Install::MacApp { bundle } => {
                // To the Trash, under a name of its own there.
                let name = format!(
                    "Clairlune {}.app",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs())
                );
                s.push_str(&format!(
                    "mkdir -p \"$HOME/.Trash\" && mv {} \"$HOME/.Trash/\"{}; ",
                    sh_quote(&bundle.to_string_lossy()),
                    sh_quote(&name)
                ));
            }
            Install::AppImage { file } => {
                s.push_str(&format!("rm -f {}; ", sh_quote(&file.to_string_lossy())));
            }
            _ => {}
        }
        let mut c = std::process::Command::new("/bin/sh");
        c.arg("-c").arg(s);
        c
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    no_window(&mut cmd);
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Only a folder named `clairlune` (any case) is ever deleted whole.
fn is_clairlune_dir(dir: &Path) -> bool {
    dir.file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("clairlune"))
        && dir.components().count() > 2
}

/// PowerShell's single-quoted string.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Windows: wait for Clairlune to quit, run setup.exe silently (`/NS`: no
/// new shortcuts; the first install made them), then start it again.
fn windows_script(pid: u32, setup: &Path, exe: &Path, relaunch: Option<&[&str]>) -> String {
    let setup = ps_quote(&setup.to_string_lossy());
    let exe = ps_quote(&exe.to_string_lossy());
    let mut s = format!(
        "$ErrorActionPreference = 'SilentlyContinue'; \
         Wait-Process -Id {pid} -Timeout 120; \
         Start-Process -FilePath {setup} -ArgumentList '/S','/NS' -Wait; \
         Remove-Item -LiteralPath {setup} -Force; "
    );
    match relaunch {
        Some([]) => s.push_str(&format!("Start-Process -FilePath {exe}")),
        Some(args) => {
            let list: Vec<String> = args.iter().map(|a| ps_quote(a)).collect();
            s.push_str(&format!(
                "Start-Process -FilePath {exe} -ArgumentList {}",
                list.join(",")
            ));
        }
        None => {}
    }
    s
}

/// A POSIX shell's single-quoted string.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// macOS and Linux: wait for Clairlune to quit, move the old copy aside,
/// the new one in (putting the old back if that fails), then start it
/// again (`open` for a Mac app) and tidy up.
fn unix_swap_script(
    pid: u32,
    new: &Path,
    target: &Path,
    download: &Path,
    relaunch: Option<&[&str]>,
    mac: bool,
) -> String {
    let new = sh_quote(&new.to_string_lossy());
    let target_q = sh_quote(&target.to_string_lossy());
    let old = sh_quote(&format!("{}.old", target.to_string_lossy()));
    let download = sh_quote(&download.to_string_lossy());
    let mut s = format!(
        "i=0; while kill -0 {pid} 2>/dev/null && [ $i -lt 600 ]; do sleep 0.2; i=$((i+1)); done; \
         rm -rf {old}; \
         if mv {target_q} {old}; then \
           if mv {new} {target_q}; then rm -rf {old}; else mv {old} {target_q}; fi; \
         fi; \
         rm -f {download}; "
    );
    if let Some(args) = relaunch {
        let args: Vec<String> = args.iter().map(|a| sh_quote(a)).collect();
        if mac {
            if args.is_empty() {
                s.push_str(&format!("open {target_q}"));
            } else {
                s.push_str(&format!("open {target_q} --args {}", args.join(" ")));
            }
        } else {
            s.push_str(&format!(
                "nohup {target_q} {} >/dev/null 2>&1 &",
                args.join(" ")
            ));
        }
    }
    s
}

/// Windows: no console window flashing up for a helper.
fn no_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// `clairlune --install-latest` (for CI): installs the latest release over
/// this copy, whatever its version, without the app, then exits; the
/// helper starts the new version with `--version` (which only prints it).
/// Returns the exit code.
pub fn install_latest_now() -> i32 {
    let install = detect();
    eprintln!("install: {}", install.name());
    let release = match crate::update::fetch() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("couldn't ask GitHub: {e}");
            return 1;
        }
    };
    let Some(name) = install.asset_name(&release.version) else {
        eprintln!("{}", install.how().unwrap_or("nothing to install"));
        return 1;
    };
    let Some(asset) = release.assets.iter().find(|a| a.name == name) else {
        eprintln!("the release has no {name}");
        return 1;
    };
    eprintln!("downloading {} ({} bytes)", asset.name, asset.size);
    let prepared = match prepare(&install, asset, &release.version, &|_| {}) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("couldn't ready {}: {e}", release.version);
            return 1;
        }
    };
    match finish(&prepared, std::process::id(), Some(&["--version"])) {
        Ok(()) => {
            eprintln!("installing {} once this exits", release.version);
            0
        }
        Err(e) => {
            eprintln!("couldn't start the installer: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(exe: &str, os: &str, appimage: Option<&str>, files: &[&str]) -> Install {
        let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
        classify(
            Path::new(exe),
            os,
            appimage.map(Path::new),
            &|p| files.iter().any(|f| Path::new(f) == p),
            &|_| true,
        )
    }

    #[test]
    fn each_kind_of_install_is_told_apart() {
        // Forward slashes: every OS's Path splits them (the test runs on all).
        let setup = "C:/Users/a/AppData/Local/Clairlune/clairlune.exe";
        assert!(matches!(
            kind(
                setup,
                "windows",
                None,
                &["C:/Users/a/AppData/Local/Clairlune/uninstall.exe"]
            ),
            Install::WindowsSetup { .. }
        ));
        assert_eq!(kind(setup, "windows", None, &[]).name(), "a portable copy");
        let scoop = kind(
            r"C:\Users\a\scoop\apps\clairlune\current\clairlune.exe",
            "windows",
            None,
            &[],
        );
        assert_eq!(scoop.how(), Some("Update it with: scoop update clairlune"));
        let cargo = kind(r"C:\Users\a\.cargo\bin\clairlune.exe", "windows", None, &[]);
        assert_eq!(cargo.name(), "cargo install");
        assert_eq!(
            kind(
                "/Applications/Clairlune.app/Contents/MacOS/clairlune",
                "macos",
                None,
                &[]
            ),
            Install::MacApp {
                bundle: PathBuf::from("/Applications/Clairlune.app")
            }
        );
        assert_eq!(
            kind(
                "/tmp/.mount_x/usr/bin/clairlune",
                "linux",
                Some("/home/a/Clairlune.AppImage"),
                &[]
            ),
            Install::AppImage {
                file: PathBuf::from("/home/a/Clairlune.AppImage")
            }
        );
        assert_eq!(
            kind("/usr/bin/clairlune", "linux", None, &[]).name(),
            "the .deb"
        );
    }

    #[test]
    fn the_download_is_the_one_for_this_install() {
        let w = Install::WindowsSetup {
            exe: PathBuf::from("x"),
        };
        assert_eq!(
            w.asset_name("0.16.0").as_deref(),
            Some("clairlune_0.16.0_x64-setup.exe")
        );
        let a = Install::AppImage {
            file: PathBuf::from("x"),
        };
        assert_eq!(
            a.asset_name("0.16.0").as_deref(),
            Some("clairlune_0.16.0_x86_64.AppImage")
        );
        let m = Install::MacApp {
            bundle: PathBuf::from("x"),
        };
        assert!(m.asset_name("0.16.0").unwrap().ends_with(".dmg"));
        assert_eq!(manual_download("x").asset_name("0.16.0"), None);
    }

    #[test]
    fn the_helpers_quote_paths_with_quotes_in_them() {
        let ps = windows_script(
            42,
            Path::new(r"C:\Temp\it's\setup.exe"),
            Path::new(r"C:\Users\o'brien\clairlune.exe"),
            Some(&["--background"]),
        );
        assert!(ps.contains(r"'C:\Temp\it''s\setup.exe'"), "{ps}");
        assert!(ps.contains("Wait-Process -Id 42"), "{ps}");
        assert!(ps.ends_with("-ArgumentList '--background'"), "{ps}");
        let sh = unix_swap_script(
            7,
            Path::new("/Apps/.new.app"),
            Path::new("/Apps/Jo's Clairlune.app"),
            Path::new("/tmp/x.dmg"),
            Some(&[]),
            true,
        );
        assert!(sh.contains(r"'/Apps/Jo'\''s Clairlune.app'"), "{sh}");
        assert!(sh.ends_with(r"open '/Apps/Jo'\''s Clairlune.app'"), "{sh}");
        assert!(
            !windows_script(1, Path::new("s"), Path::new("e"), None)
                .contains("Start-Process -FilePath 'e'")
        );
    }

    #[test]
    fn only_clairlune_folders_are_deleted_whole() {
        assert!(is_clairlune_dir(Path::new(
            "/home/a/.local/share/clairlune"
        )));
        assert!(is_clairlune_dir(Path::new(
            "C:/Users/a/AppData/Local/Clairlune"
        )));
        assert!(!is_clairlune_dir(Path::new("/home/a")));
        assert!(!is_clairlune_dir(Path::new("/clairlune")));
        assert!(!is_clairlune_dir(Path::new("C:/Users/a/AppData/Local")));
    }

    #[test]
    fn removal_says_what_goes_and_how_to_finish() {
        let w = Install::WindowsSetup {
            exe: PathBuf::from("x"),
        };
        assert!(w.removal().is_ok());
        let scoop = Install::Manual {
            name: "Scoop".into(),
            how: String::new(),
        };
        assert_eq!(
            scoop.removal(),
            Err("Then remove the app with: scoop uninstall clairlune".to_string())
        );
    }

    #[test]
    fn a_file_hashes_like_sha256() {
        let dir = std::env::temp_dir().join(format!("clairlune-sha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("abc");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Downloads this platform's file of the latest release for real and
    /// checks it (nothing is installed):
    /// `cargo test -p clairlune install -- --ignored`.
    #[test]
    #[ignore]
    fn the_latest_download_checks_out() {
        let r = crate::update::fetch().unwrap();
        let install = if cfg!(windows) {
            Install::WindowsSetup {
                exe: PathBuf::from("clairlune.exe"),
            }
        } else if cfg!(target_os = "macos") {
            Install::MacApp {
                bundle: std::env::temp_dir().join("Clairlune.app"),
            }
        } else {
            Install::AppImage {
                file: std::env::temp_dir().join("Clairlune.AppImage"),
            }
        };
        let name = install.asset_name(&r.version).unwrap();
        let asset = r.assets.iter().find(|a| a.name == name).unwrap();
        let seen = std::cell::RefCell::new(Vec::new());
        let p = prepare(&install, asset, &r.version, &|n| seen.borrow_mut().push(n)).unwrap();
        assert_eq!(seen.borrow().last(), Some(&100));
        eprintln!("ready: {p:?}");
        let _ = std::fs::remove_file(&p.file);
        if let Some(s) = &p.staged {
            let _ = std::fs::remove_dir_all(s);
        }
    }
}
