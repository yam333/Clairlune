//! The Clairlune desktop app. The tray, settings and live wallpapers arrive in
//! milestone M6; until then this binary only explains where things stand.

use clairlune_scene::{APP_ID, APP_NAME};

fn main() {
    println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
    println!("The desktop app ({APP_ID}) is not built yet: it arrives in milestone M6.");
    println!("To render a wallpaper now, use the developer CLI:");
    println!("  clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out wallpaper.png");
}
