//! A second launch asks the running copy to open Settings, over a local
//! socket: a named pipe on Windows, an abstract or file
//! socket elsewhere. The name is derived from the user's data folder, so two
//! users on one machine never reach each other's copy.

use clairlune_scene::hash::hash_bytes;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Name, Stream, prelude::*,
};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

const OPEN_SETTINGS: &str = "open-settings";

fn name(data_dir: &Path) -> io::Result<Name<'static>> {
    if GenericNamespaced::is_supported() {
        let key = hash_bytes(data_dir.to_string_lossy().as_bytes());
        format!("clairlune-{key:016x}.sock")
            .to_ns_name::<GenericNamespaced>()
            .map(Name::into_owned)
    } else {
        data_dir
            .join("ipc.sock")
            .to_fs_name::<GenericFilePath>()
            .map(Name::into_owned)
    }
}

/// Listens for requests on a background thread; `on_open_settings` runs on
/// that thread, so it should only forward to the event loop.
pub fn serve(data_dir: &Path, on_open_settings: impl Fn() + Send + 'static) -> io::Result<()> {
    let listener = ListenerOptions::new()
        .name(name(data_dir)?)
        .try_overwrite(true)
        .create_sync()?;
    std::thread::Builder::new()
        .name("clairlune-ipc".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut line = String::new();
                if BufReader::new(conn).read_line(&mut line).is_ok() && line.trim() == OPEN_SETTINGS
                {
                    on_open_settings();
                }
            }
        })?;
    Ok(())
}

/// Asks the running copy to open Settings; false if none answered.
pub fn ask_running_copy(data_dir: &Path) -> bool {
    name(data_dir)
        .and_then(Stream::connect)
        .and_then(|mut s| s.write_all(format!("{OPEN_SETTINGS}\n").as_bytes()))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_request_reaches_the_listener() {
        let dir = std::env::temp_dir().join(format!("clairlune-ipc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = mpsc::channel();
        serve(&dir, move || {
            let _ = tx.send(());
        })
        .unwrap();
        assert!(ask_running_copy(&dir));
        rx.recv_timeout(Duration::from_secs(5))
            .expect("request received");
        // Someone else's folder has no listener.
        assert!(!ask_running_copy(&dir.join("nobody")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
