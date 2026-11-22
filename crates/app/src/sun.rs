//! Sunrise and sunset, worked out on this computer from a latitude and
//! longitude. NOAA's general solar
//! position equations: within a couple of minutes away from the poles,
//! which is plenty for switching the wallpaper's theme.

use jiff::civil::Date;
use jiff::{SignedDuration, Timestamp};
use std::f64::consts::PI;

/// The sun over a day at a place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Daylight {
    /// It rises and sets.
    Normal { rise: Timestamp, set: Timestamp },
    /// It doesn't set (polar summer).
    AlwaysUp,
    /// It doesn't rise (polar winter).
    AlwaysDown,
}

/// Sunrise and sunset on `date` (the local date) at `lat`, `lon` in degrees
/// (north and east positive).
pub fn daylight(date: Date, lat: f64, lon: f64) -> Daylight {
    let day = f64::from(date.day_of_year());
    let days_in_year = if date.in_leap_year() { 366.0 } else { 365.0 };
    // Fractional year at noon, in radians.
    let g = 2.0 * PI / days_in_year * (day - 1.0);
    let eqtime = 229.18
        * (0.000075 + 0.001868 * g.cos()
            - 0.032077 * g.sin()
            - 0.014615 * (2.0 * g).cos()
            - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos()
        + 0.000907 * (2.0 * g).sin()
        - 0.002697 * (3.0 * g).cos()
        + 0.00148 * (3.0 * g).sin();
    let phi = lat.to_radians();
    // 90.833 degrees: the sun's upper edge on the horizon, with refraction.
    let cos_ha = 90.833f64.to_radians().cos() / (phi.cos() * decl.cos()) - phi.tan() * decl.tan();
    if cos_ha > 1.0 {
        return Daylight::AlwaysDown;
    }
    if cos_ha < -1.0 {
        return Daylight::AlwaysUp;
    }
    let ha = cos_ha.acos().to_degrees();
    // Minutes after midnight UTC on `date`; may fall on the day before or after.
    let rise = 720.0 - 4.0 * (lon + ha) - eqtime;
    let set = 720.0 - 4.0 * (lon - ha) - eqtime;
    let midnight = date
        .to_zoned_datetime(jiff::tz::TimeZone::UTC)
        .expect("a valid date")
        .timestamp();
    let at = |minutes: f64| midnight + SignedDuration::from_secs((minutes * 60.0).round() as i64);
    Daylight::Normal {
        rise: at(rise),
        set: at(set),
    }
}

/// Whether the sun is up at `now` at a place. The day is the place's own
/// (from its longitude), not this computer's: the two differ when the
/// computer's time zone is another one's.
pub fn is_day(lat: f64, lon: f64, now: Timestamp) -> bool {
    // Local mean solar time there: 4 minutes per degree east of Greenwich.
    let solar = now + SignedDuration::from_secs((lon * 240.0).round() as i64);
    let date = solar.to_zoned_datetime(jiff::tz::TimeZone::UTC).date();
    match daylight(date, lat, lon) {
        Daylight::Normal { rise, set } => rise <= now && now < set,
        Daylight::AlwaysUp => true,
        Daylight::AlwaysDown => false,
    }
}

/// A location typed as "latitude, longitude" (decimal degrees, south and
/// west negative), e.g. "40.71, -74.01".
pub fn parse_location(text: &str) -> Option<(f64, f64)> {
    let mut parts = text.split([',', ';', ' ']).filter(|p| !p.is_empty());
    let lat: f64 = parts.next()?.trim().parse().ok()?;
    let lon: f64 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon)
    {
        return None;
    }
    Some((lat, lon))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;
    use jiff::tz::TimeZone;

    /// Local clock time of an instant, as minutes after midnight.
    fn local_minutes(t: Timestamp, tz: &str) -> i64 {
        let z = t.to_zoned_datetime(TimeZone::get(tz).unwrap());
        i64::from(z.hour()) * 60 + i64::from(z.minute())
    }

    fn assert_near(got: i64, want: (i64, i64), what: &str) {
        let want = want.0 * 60 + want.1;
        assert!(
            (got - want).abs() <= 3,
            "{what}: {}:{:02}",
            got / 60,
            got % 60
        );
    }

    #[test]
    fn new_york_at_midsummer() {
        let Daylight::Normal { rise, set } = daylight(date(2026, 6, 21), 40.7128, -74.0060) else {
            panic!("the sun rises in New York");
        };
        assert_near(local_minutes(rise, "America/New_York"), (5, 25), "sunrise");
        assert_near(local_minutes(set, "America/New_York"), (20, 31), "sunset");
    }

    #[test]
    fn london_at_midwinter() {
        let Daylight::Normal { rise, set } = daylight(date(2026, 12, 21), 51.5074, -0.1278) else {
            panic!("the sun rises in London");
        };
        assert_near(local_minutes(rise, "Europe/London"), (8, 4), "sunrise");
        assert_near(local_minutes(set, "Europe/London"), (15, 53), "sunset");
    }

    #[test]
    fn tokyo_sunrise_falls_on_the_utc_day_before() {
        let Daylight::Normal { rise, set } = daylight(date(2026, 6, 21), 35.6762, 139.6503) else {
            panic!("the sun rises in Tokyo");
        };
        assert_near(local_minutes(rise, "Asia/Tokyo"), (4, 25), "sunrise");
        assert_near(local_minutes(set, "Asia/Tokyo"), (19, 0), "sunset");
        assert!(rise < set);
    }

    #[test]
    fn polar_day_and_night() {
        assert_eq!(
            daylight(date(2026, 6, 21), 69.65, 18.96),
            Daylight::AlwaysUp
        );
        assert_eq!(
            daylight(date(2026, 12, 21), 69.65, 18.96),
            Daylight::AlwaysDown
        );
    }

    #[test]
    fn day_and_night_at_a_place() {
        let at = |h: i8, m: i8| {
            date(2026, 6, 21)
                .at(h, m, 0, 0)
                .to_zoned_datetime(TimeZone::get("America/New_York").unwrap())
                .unwrap()
                .timestamp()
        };
        assert!(!is_day(40.7128, -74.0060, at(5, 0)));
        assert!(is_day(40.7128, -74.0060, at(12, 0)));
        assert!(is_day(40.7128, -74.0060, at(20, 20)));
        assert!(!is_day(40.7128, -74.0060, at(21, 0)));
    }

    #[test]
    fn the_place_keeps_its_own_day_whatever_the_computer_clock() {
        // Tokyo, 06:00 there on June 21 is 21:00 UTC on June 20: a computer
        // on UTC has the wrong date for Tokyo.
        let tokyo = |h: i8| {
            date(2026, 6, 21)
                .at(h, 0, 0, 0)
                .to_zoned_datetime(TimeZone::get("Asia/Tokyo").unwrap())
                .unwrap()
                .timestamp()
        };
        assert!(is_day(35.6762, 139.6503, tokyo(6)));
        assert!(is_day(35.6762, 139.6503, tokyo(12)));
        assert!(!is_day(35.6762, 139.6503, tokyo(2)));
        assert!(!is_day(35.6762, 139.6503, tokyo(22)));
        // Los Angeles, 18:00 there is the next UTC day.
        let la = |h: i8| {
            date(2026, 6, 21)
                .at(h, 0, 0, 0)
                .to_zoned_datetime(TimeZone::get("America/Los_Angeles").unwrap())
                .unwrap()
                .timestamp()
        };
        assert!(is_day(34.05, -118.24, la(18)));
        assert!(!is_day(34.05, -118.24, la(23)));
        assert!(!is_day(34.05, -118.24, la(4)));
    }

    #[test]
    fn locations_parse() {
        assert_eq!(parse_location("40.71, -74.01"), Some((40.71, -74.01)));
        assert_eq!(parse_location(" -33.87;151.21 "), Some((-33.87, 151.21)));
        assert_eq!(parse_location("51.5 -0.13"), Some((51.5, -0.13)));
        assert_eq!(parse_location("91, 0"), None);
        assert_eq!(parse_location("40.71"), None);
        assert_eq!(parse_location("north, west"), None);
        assert_eq!(parse_location("1, 2, 3"), None);
    }
}
