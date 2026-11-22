//! Whether the computer runs on battery: Windows asks the power status,
//! macOS its power sources (IOKit). Unknown means mains, so nothing slows
//! down by mistake.

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
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
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

    #[test]
    fn on_battery_answers() {
        // Whatever the machine: it answers without panicking.
        let _ = on_battery();
    }
}
