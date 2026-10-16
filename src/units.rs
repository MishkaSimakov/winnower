//! Human-readable sizes and ages.

use std::time::{Duration, SystemTime};

/// Size units after bytes, each 1000 times the previous one.
const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];

/// Formats `bytes` as `512 B` below one kilobyte, otherwise with one decimal
/// in the largest unit that keeps the number below 1000, for example `1.2 GB`.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 999.95 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Formats the number of whole days from `then` to `now`, for example `94 days ago`.
/// A `then` later than `now` counts as zero days.
pub fn format_days_ago(then: SystemTime, now: SystemTime) -> String {
    let days = now.duration_since(then).unwrap_or(Duration::ZERO).as_secs() / (24 * 60 * 60);
    if days == 1 {
        "1 day ago".to_string()
    } else {
        format!("{days} days ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    // Sizes below one kilobyte are shown in bytes without decimals.
    #[test]
    fn bytes_are_shown_without_decimals() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(999), "999 B");
    }

    // Larger sizes use KB, MB, GB, and TB with one decimal.
    #[test]
    fn larger_sizes_use_the_right_unit() {
        assert_eq!(format_size(1000), "1.0 KB");
        assert_eq!(format_size(12_100_000), "12.1 MB");
        assert_eq!(format_size(310_000_000), "310.0 MB");
        assert_eq!(format_size(2_300_000_000), "2.3 GB");
        assert_eq!(format_size(4_000_000_000_000), "4.0 TB");
    }

    // A size that would round to 1000 of one unit is shown in the next unit.
    #[test]
    fn rounding_moves_to_the_next_unit() {
        assert_eq!(format_size(999_960), "1.0 MB");
    }

    // Ages count whole days and use the singular for one day.
    #[test]
    fn days_ago_counts_whole_days() {
        let now = SystemTime::now();
        assert_eq!(format_days_ago(now, now), "0 days ago");
        assert_eq!(format_days_ago(now - DAY, now), "1 day ago");
        assert_eq!(
            format_days_ago(now - DAY * 94 - DAY / 2, now),
            "94 days ago"
        );
    }

    // A time in the future counts as zero days ago.
    #[test]
    fn future_time_is_zero_days_ago() {
        let now = SystemTime::now();
        assert_eq!(format_days_ago(now + DAY, now), "0 days ago");
    }
}
