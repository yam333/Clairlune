//! Clairlune's macOS screen saver (`Clairlune.saver`): when the Mac idles,
//! the scene on the desktop keeps playing, full screen.
//!
//! The bundle's executable is this library; its principal class,
//! `ClairluneSaverView`, is a `ScreenSaverView` defined here and registered
//! with the Objective-C runtime as the library loads (the screen saver host
//! looks it up by name). Each screen gets a view; each view plays the scene
//! the app last set on a screen of its size (`screensaver.json`, written by
//! the app), shown through IOSurfaces like the live wallpaper. Without
//! that file (the app never ran, or the sandbox the host runs savers in
//! keeps it out), the saver makes a scene of its own.
//!
//! Checked in CI: a macOS runner loads the bundle, plays it and takes
//! screenshots a moment apart, which must differ. It notes what it plays
//! in `Library/Logs/Clairlune Saver.log` of the home the host gives it.

#[cfg(target_os = "macos")]
mod view;
