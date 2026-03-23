use loopal_tool_api::{FileInfo, LsEntry};
use std::time::SystemTime;

/// Format a long-mode line from an `LsEntry`.
pub fn format_long_from_entry(entry: &LsEntry, indicator: &str) -> String {
    let prefix = if entry.is_dir {
        'd'
    } else if entry.is_symlink {
        'l'
    } else {
        '-'
    };
    let perms = format_permission_bits(prefix, entry.permissions);
    let size = format_size(entry.size);
    let mtime = entry
        .modified
        .map(format_epoch)
        .unwrap_or_else(|| "????-??-?? ??:??".into());
    format!("{perms}  {size:>6}  {mtime}  {}{indicator}", entry.name)
}

/// Format stat-like output from `FileInfo`.
pub fn format_stat_from_info(path: &std::path::Path, info: &FileInfo) -> String {
    let file_type = if info.is_dir {
        "directory"
    } else {
        "regular file"
    };
    let mtime = info
        .modified
        .map(format_epoch)
        .unwrap_or_else(|| "unknown".into());
    format!(
        "File: {}\nType: {}\nSize: {} bytes ({})\nModified: {}",
        path.display(),
        file_type,
        info.size,
        format_size(info.size),
        mtime,
    )
}

/// Format Unix permission bits as `drwxr-xr-x`.
fn format_permission_bits(prefix: char, mode: Option<u32>) -> String {
    let Some(mode) = mode else {
        return format!("{prefix}---------");
    };
    let mut s = String::with_capacity(10);
    s.push(prefix);
    for shift in (0..3).rev() {
        let bits = (mode >> (shift * 3)) & 7;
        s.push(if bits & 4 != 0 { 'r' } else { '-' });
        s.push(if bits & 2 != 0 { 'w' } else { '-' });
        s.push(if bits & 1 != 0 { 'x' } else { '-' });
    }
    s
}

/// Human-readable file size: `123B`, `1.2K`, `3.4M`, `1.0G`.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1_048_576 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1}M", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    }
}

/// Format a `SystemTime` as `YYYY-MM-DD HH:MM`.
pub fn format_time(time: SystemTime) -> String {
    let secs = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_epoch(secs)
}

/// Format unix epoch seconds as `YYYY-MM-DD HH:MM`.
fn format_epoch(epoch: u64) -> String {
    let (y, m, d, h, min) = epoch_to_datetime(epoch);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}")
}

/// Convert unix epoch seconds to (year, month, day, hour, minute).
/// Uses the Howard Hinnant civil_from_days algorithm.
fn epoch_to_datetime(epoch: u64) -> (i32, u32, u32, u32, u32) {
    let total_days = (epoch / 86400) as i64;
    let time_of_day = epoch % 86400;
    let z = total_days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        y as i32,
        m,
        d,
        (time_of_day / 3600) as u32,
        ((time_of_day % 3600) / 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_formatting() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1_048_576), "1.0M");
        assert_eq!(format_size(1_073_741_824), "1.0G");
    }

    #[test]
    fn time_formatting() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_704_067_200);
        assert_eq!(format_time(t), "2024-01-01 00:00");
    }

    #[test]
    fn epoch_known_date() {
        // 1970-01-01 00:00
        assert_eq!(epoch_to_datetime(0), (1970, 1, 1, 0, 0));
        // 2000-01-01 12:30 UTC = 946729800
        assert_eq!(epoch_to_datetime(946_729_800), (2000, 1, 1, 12, 30));
    }
}
