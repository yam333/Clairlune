// Embeds the Windows application manifest (MSVC linker), and stops a build
// for a system the app doesn't run on.
fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os != "windows" && os != "macos" {
        eprintln!(
            "Clairlune's desktop app runs on Windows and macOS, not on {os}. \
             The scenes render anywhere with clairlune-cli: cargo install clairlune-cli"
        );
        std::process::exit(1);
    }
    println!("cargo:rerun-if-changed=res/clairlune.manifest");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("res")
            .join("clairlune.manifest");
        // The moon as the exe's icon (Start menu, taskbar, Apps list).
        println!("cargo:rerun-if-changed=res/clairlune.rc");
        println!("cargo:rerun-if-changed=packaging/icons/clairlune.ico");
        // The build script runs on the build machine: only a Windows one has
        // the resource compiler (and this dependency).
        #[cfg(windows)]
        let _ = embed_resource::compile("res/clairlune.rc", embed_resource::NONE);
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
