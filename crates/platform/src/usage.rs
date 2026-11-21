//! What this process uses, for the log: CPU time so far, and memory now
//! (on macOS the footprint Activity Monitor shows, GPU surfaces included).

use std::time::Duration;

/// CPU time (user and system) the process has used so far.
pub fn process_cpu_time() -> Duration {
    #[cfg(windows)]
    {
        crate::windows_live::process_cpu_time()
    }
    #[cfg(unix)]
    {
        // SAFETY: getrusage fills the struct it is given.
        let mut r: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut r) } != 0 {
            return Duration::ZERO;
        }
        let tv = |t: libc::timeval| {
            Duration::from_secs(t.tv_sec.max(0) as u64)
                + Duration::from_micros(t.tv_usec.max(0) as u64)
        };
        tv(r.ru_utime) + tv(r.ru_stime)
    }
    #[cfg(not(any(windows, unix)))]
    {
        Duration::ZERO
    }
}

/// Memory the process holds now, in bytes (0 where unknown).
pub fn process_memory() -> u64 {
    #[cfg(windows)]
    {
        crate::windows_live::process_memory()
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: proc_pid_rusage fills a rusage_info_v2 for this flavour.
        let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            libc::proc_pid_rusage(
                libc::getpid(),
                libc::RUSAGE_INFO_V2,
                (&mut info as *mut libc::rusage_info_v2).cast(),
            )
        } == 0;
        if ok { info.ri_phys_footprint } else { 0 }
    }
    #[cfg(target_os = "linux")]
    {
        // Resident pages, the second field of /proc/self/statm.
        let pages = std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
            .unwrap_or(0);
        // SAFETY: sysconf has no preconditions.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(0) as u64;
        pages * page
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_uses_something() {
        // Burn CPU until the clock moves (Windows counts in 15.6 ms ticks).
        let start = std::time::Instant::now();
        let mut x = 0u64;
        while process_cpu_time() == Duration::ZERO && start.elapsed() < Duration::from_secs(5) {
            for i in 0..1_000_000u64 {
                x = std::hint::black_box(x.wrapping_mul(31).wrapping_add(i));
            }
        }
        assert!(process_cpu_time() > Duration::ZERO);
        assert!(process_memory() > 0);
    }
}
