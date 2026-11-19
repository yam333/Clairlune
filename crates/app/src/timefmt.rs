//! UTC timestamps for archive entry names, without a date library: days since
//! the epoch to a civil date (Howard Hinnant's algorithm). UTC so daylight
//! saving can't produce two entries with the same name.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch, now.
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `(year, month, day)` for a count of days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2026-11-18T17-56-03Z`: ISO 8601 with `-` for `:` so it is a valid file name.
pub fn stamp(unix: u64) -> String {
    let secs = unix as i64;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}-{:02}-{:02}Z",
        s / 3600,
        (s % 3600) / 60,
        s % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants() {
        assert_eq!(stamp(0), "1970-01-01T00-00-00Z");
        assert_eq!(stamp(951_782_400), "2000-02-29T00-00-00Z");
        assert_eq!(stamp(1_795_542_963), "2026-11-24T17-56-03Z");
        assert_eq!(stamp(4_102_444_799), "2099-12-31T23-59-59Z");
    }

    #[test]
    fn stamps_sort_like_time() {
        let mut v: Vec<String> = [5u64, 1_000_000_000, 86_399, 86_400, 2_000_000_000]
            .iter()
            .map(|&t| stamp(t))
            .collect();
        let sorted = {
            let mut c = v.clone();
            c.sort();
            c
        };
        v.sort_by_key(|s| s.clone());
        assert_eq!(v, sorted);
        assert!(stamp(86_399) < stamp(86_400));
    }
}
