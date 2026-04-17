//! Compact relative-time formatter for the Crons panel's "next fire" field.
//!
//! Output length ≤ 10 characters so the suffix `next Xm Ys  [R]` fits on
//! narrow terminals without forcing subject truncation.

use chrono::{DateTime, Utc};

/// Format `next` relative to `now` as a compact suffix.
///
/// - `None`    → `"—"`
/// - past time → `"now"`
/// - `< 60s`   → `"Xs"`
/// - `< 1h`    → `"Xm Ys"`
/// - `< 1d`    → `"Xh Ym"`
/// - `>= 1d`   → `"Xd Yh"`
pub fn format_next_fire(next: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    let Some(t) = next else { return "—".into() };
    let secs = t.signed_duration_since(now).num_seconds();
    if secs <= 0 {
        return "now".into();
    }
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        return format!("{m}m {s}s");
    }
    if secs < 86_400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        return format!("{h}h {m}m");
    }
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    format!("{d}d {h}h")
}

/// Unix-ms convenience wrapper — input format of `CronJobSnapshot`.
pub fn format_next_fire_ms(next_ms: Option<i64>, now: DateTime<Utc>) -> String {
    let dt = next_ms.and_then(DateTime::<Utc>::from_timestamp_millis);
    format_next_fire(dt, now)
}
