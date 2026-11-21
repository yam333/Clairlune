//! Whether the computer runs on battery: Windows asks the power status,
//! macOS its power sources (IOKit), Linux reads `/sys/class/power_supply`.
//! Unknown means mains, so nothing slows down by mistake.

/// Running on battery power.
pub fn on_battery() -> bool {
    #[cfg(windows)]
    {
        crate::windows_live::on_battery()
    }
    #[cfg(target_os = "macos")]
    {
        mac::on_battery()
    }
    #[cfg(target_os = "linux")]
    {
        linux_on_battery(&linux_supplies())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// One power supply as Linux lists it: its `type`, and `online` (mains)
/// or `status` (batteries).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Default)]
pub(crate) struct Supply {
    pub kind: String,
    pub online: Option<bool>,
    pub status: String,
}

/// On battery: a system battery and no mains supply online. A battery that
/// says it's discharging counts even when no mains supply is listed.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn linux_on_battery(supplies: &[Supply]) -> bool {
    let mains_online = supplies
        .iter()
        .any(|s| s.kind == "Mains" && s.online == Some(true));
    if mains_online {
        return false;
    }
    let batteries: Vec<&Supply> = supplies.iter().filter(|s| s.kind == "Battery").collect();
    let any_mains = supplies.iter().any(|s| s.kind == "Mains");
    !batteries.is_empty() && (any_mains || batteries.iter().any(|b| b.status == "Discharging"))
}

#[cfg(target_os = "linux")]
fn linux_supplies() -> Vec<Supply> {
    let read = |p: std::path::PathBuf| {
        std::fs::read_to_string(p)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let Ok(dir) = std::fs::read_dir("/sys/class/power_supply") else {
        return Vec::new();
    };
    dir.flatten()
        .map(|e| {
            let p = e.path();
            // A device's own battery (a mouse, a headset) isn't the system's.
            let scope = read(p.join("scope"));
            let kind = if scope == "Device" {
                String::new()
            } else {
                read(p.join("type"))
            };
            Supply {
                kind,
                online: match read(p.join("online")).as_str() {
                    "1" => Some(true),
                    "0" => Some(false),
                    _ => None,
                },
                status: read(p.join("status")),
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{CStr, c_char, c_void};

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        /// A snapshot of the power sources (a CFTypeRef the caller releases).
        fn IOPSCopyPowerSourcesInfo() -> *const c_void;
        /// "AC Power", "Battery Power" or "Off Line Power" (a CFStringRef
        /// owned by the snapshot).
        fn IOPSGetProvidingPowerSourceType(snapshot: *const c_void) -> *const c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFStringGetCString(
            s: *const c_void,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
    }

    const UTF8: u32 = 0x0800_0100;

    pub(super) fn on_battery() -> bool {
        // SAFETY: the snapshot is checked for null and released once; the
        // string belongs to it and is copied out before the release.
        unsafe {
            let snapshot = IOPSCopyPowerSourcesInfo();
            if snapshot.is_null() {
                return false;
            }
            let kind = IOPSGetProvidingPowerSourceType(snapshot);
            let mut buf = [0 as c_char; 64];
            let ok = !kind.is_null()
                && CFStringGetCString(kind, buf.as_mut_ptr(), buf.len() as isize, UTF8) != 0;
            CFRelease(snapshot);
            ok && CStr::from_ptr(buf.as_ptr()).to_bytes() == b"Battery Power"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(kind: &str, online: Option<bool>, status: &str) -> Supply {
        Supply {
            kind: kind.into(),
            online,
            status: status.into(),
        }
    }

    #[test]
    fn linux_power_from_its_supplies() {
        // A laptop plugged in, and unplugged.
        assert!(!linux_on_battery(&[
            s("Mains", Some(true), ""),
            s("Battery", None, "Charging")
        ]));
        assert!(linux_on_battery(&[
            s("Mains", Some(false), ""),
            s("Battery", None, "Discharging")
        ]));
        // A desktop: no battery, mains or not listed.
        assert!(!linux_on_battery(&[s("Mains", Some(true), "")]));
        assert!(!linux_on_battery(&[]));
        // No mains listed, a battery running down.
        assert!(linux_on_battery(&[s("Battery", None, "Discharging")]));
        assert!(!linux_on_battery(&[s("Battery", None, "Full")]));
        // A mouse's battery (scope Device) is listed with no kind.
        assert!(!linux_on_battery(&[s("", None, "Discharging")]));
    }

    #[test]
    fn on_battery_answers() {
        // Whatever the machine: it answers without panicking.
        let _ = on_battery();
    }
}
