//! Date utilities for the memory system.
//!
//! Pure functions with no I/O dependencies — safe to use in any crate.

/// Format today's date as YYYY-MM-DD.
pub fn today_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    epoch_days_to_date((secs / 86400) as i64)
}

/// Convert days since Unix epoch to YYYY-MM-DD string.
///
/// Uses the civil calendar conversion algorithm by Howard Hinnant.
/// Reference: <https://howardhinnant.github.io/date_algorithms.html>
pub fn epoch_days_to_date(days: i64) -> String {
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
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse YYYY-MM-DD and return days since Unix epoch, or None on parse error.
pub fn parse_date_to_days(date: &str) -> Option<i64> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let d: u64 = parts[2].parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe as i64 - 719468)
}

/// Return the number of days between two YYYY-MM-DD dates.
pub fn days_between(from: &str, to: &str) -> Option<i64> {
    let f = parse_date_to_days(from)?;
    let t = parse_date_to_days(to)?;
    Some(t - f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_roundtrip() {
        let date_str = epoch_days_to_date(20556);
        assert_eq!(date_str, "2026-04-13");
        assert_eq!(parse_date_to_days("2026-04-13"), Some(20556));
    }

    #[test]
    fn test_days_between() {
        assert_eq!(days_between("2026-04-01", "2026-04-13"), Some(12));
        assert_eq!(days_between("2026-04-13", "2026-04-13"), Some(0));
        assert_eq!(days_between("2026-04-13", "2026-04-06"), Some(-7));
    }

    #[test]
    fn test_parse_date_invalid_input() {
        assert_eq!(parse_date_to_days("not-a-date"), None);
        assert_eq!(parse_date_to_days("2026"), None);
        assert_eq!(parse_date_to_days(""), None);
    }

    #[test]
    fn test_days_between_cross_year_and_leap() {
        assert_eq!(days_between("2025-12-31", "2026-01-01"), Some(1));
        assert_eq!(days_between("2024-02-28", "2024-03-01"), Some(2)); // leap
        assert_eq!(days_between("2025-02-28", "2025-03-01"), Some(1)); // non-leap
    }
}
