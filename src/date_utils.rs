//! Shared date/time utility functions.
//!
//! Centralises calendar arithmetic (Howard Hinnant's civil calendar algorithms)
//! and ISO-8601 formatting that were previously duplicated across blame, report,
//! clean, and watch modules.

/// Convert days since Unix epoch to (year, month, day).
///
/// Algorithm based on `civil_from_days` by Howard Hinnant.
pub fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

/// Convert (year, month, day) to days since Unix epoch.
///
/// Inverse of [`days_to_ymd`]. Algorithm based on Howard Hinnant's civil calendar.
pub fn ymd_to_days(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 {
        month as i64 + 9
    } else {
        month as i64 - 3
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * m as u64 + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

/// Format a Unix epoch timestamp (seconds) as an ISO-8601 UTC string.
///
/// Returns a string in the form `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_iso8601_utc(epoch_secs: u64) -> String {
    let days = epoch_secs / 86400;
    let (year, month, day) = days_to_ymd(days as i64);
    let secs_today = epoch_secs % 86400;
    let hour = secs_today / 3600;
    let minute = (secs_today % 3600) / 60;
    let second = secs_today % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Return the current UTC time as an ISO-8601 string.
///
/// Convenience wrapper around [`format_iso8601_utc`] using the system clock.
pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_iso8601_utc(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── days_to_ymd ──────────────────────────────────────────

    #[test]
    fn days_to_ymd_epoch_zero() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2024-01-01 is 19723 days after epoch
        assert_eq!(days_to_ymd(19723), (2024, 1, 1));
    }

    #[test]
    fn days_to_ymd_negative_days() {
        // 1969-12-31 is day -1
        assert_eq!(days_to_ymd(-1), (1969, 12, 31));
    }

    // ── ymd_to_days ──────────────────────────────────────────

    #[test]
    fn ymd_to_days_epoch() {
        assert_eq!(ymd_to_days(1970, 1, 1), 0);
    }

    #[test]
    fn ymd_to_days_known_date() {
        assert_eq!(ymd_to_days(2024, 1, 1), 19723);
    }

    // ── roundtrip ────────────────────────────────────────────

    #[test]
    fn roundtrip_days_ymd() {
        for days in [-365, -1, 0, 1, 365, 10000, 19723, 20000] {
            let (y, m, d) = days_to_ymd(days);
            assert_eq!(
                ymd_to_days(y, m, d),
                days,
                "roundtrip failed for day {days}"
            );
        }
    }

    // ── format_iso8601_utc ───────────────────────────────────

    #[test]
    fn format_iso8601_utc_epoch_zero() {
        assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn format_iso8601_utc_known_timestamp() {
        // 2024-01-01 11:50:45 UTC = 1704109845
        assert_eq!(format_iso8601_utc(1704109845), "2024-01-01T11:50:45Z");
    }

    #[test]
    fn days_to_ymd_far_negative() {
        // Test negative era calculation (z < 0)
        let (y, m, d) = days_to_ymd(-719162);
        assert!(y < 100); // Very early year
                          // Roundtrip to confirm correctness
        assert_eq!(ymd_to_days(y, m, d), -719162);
    }

    #[test]
    fn ymd_to_days_negative_year() {
        // Test year before 0 to hit the (y - 399) / 400 branch
        let days = ymd_to_days(-100, 6, 15);
        let (y, m, d) = days_to_ymd(days);
        assert_eq!(y, -100);
        assert_eq!(m, 6);
        assert_eq!(d, 15);
    }

    #[test]
    fn ymd_to_days_january_adjusts_year() {
        // Month <= 2 hits the year-1 and month+9 branches
        let jan = ymd_to_days(2025, 1, 15);
        let feb = ymd_to_days(2025, 2, 15);
        let mar = ymd_to_days(2025, 3, 15);
        assert!(jan < feb);
        assert!(feb < mar);
    }

    #[test]
    fn ymd_to_days_roundtrip_leap_year() {
        let days = ymd_to_days(2024, 2, 29);
        let (y, m, d) = days_to_ymd(days);
        assert_eq!((y, m, d), (2024, 2, 29));
    }

    #[test]
    fn format_iso8601_utc_end_of_day() {
        assert_eq!(format_iso8601_utc(86399), "1970-01-01T23:59:59Z");
    }

    // ── now_iso8601 ──────────────────────────────────────────

    #[test]
    fn now_iso8601_format() {
        let ts = now_iso8601();
        assert!(ts.ends_with('Z'), "should end with Z: {ts}");
        assert_eq!(ts.len(), 20, "ISO-8601 UTC should be 20 chars: {ts}");
        assert!(ts.contains('T'), "should contain T separator: {ts}");
    }
}
