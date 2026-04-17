//! Tests for cron_duration_format — relative time formatting.

use chrono::{DateTime, Duration, Utc};
use loopal_tui::views::cron_duration_format::{format_next_fire, format_next_fire_ms};

fn base_now() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap()
}

#[test]
fn none_returns_dash() {
    assert_eq!(format_next_fire(None, base_now()), "—");
}

#[test]
fn past_returns_now() {
    let now = base_now();
    let past = now - Duration::seconds(10);
    assert_eq!(format_next_fire(Some(past), now), "now");
}

#[test]
fn zero_delta_returns_now() {
    let now = base_now();
    assert_eq!(format_next_fire(Some(now), now), "now");
}

#[test]
fn seconds_only() {
    let now = base_now();
    let future = now + Duration::seconds(45);
    assert_eq!(format_next_fire(Some(future), now), "45s");
}

#[test]
fn minutes_and_seconds() {
    let now = base_now();
    let future = now + Duration::seconds(150);
    assert_eq!(format_next_fire(Some(future), now), "2m 30s");
}

#[test]
fn exactly_60s_returns_1m_0s() {
    let now = base_now();
    let future = now + Duration::seconds(60);
    assert_eq!(format_next_fire(Some(future), now), "1m 0s");
}

#[test]
fn hours_and_minutes() {
    let now = base_now();
    let future = now + Duration::seconds(3_700);
    assert_eq!(format_next_fire(Some(future), now), "1h 1m");
}

#[test]
fn days_and_hours() {
    let now = base_now();
    let future = now + Duration::seconds(90_000);
    assert_eq!(format_next_fire(Some(future), now), "1d 1h");
}

#[test]
fn ms_helper_round_trip() {
    let now = base_now();
    let future_ms = (now + Duration::seconds(75)).timestamp_millis();
    assert_eq!(format_next_fire_ms(Some(future_ms), now), "1m 15s");
}

#[test]
fn ms_helper_none_returns_dash() {
    assert_eq!(format_next_fire_ms(None, base_now()), "—");
}

#[test]
fn ms_helper_i64_max_returns_dash() {
    // i64::MAX milliseconds overflows chrono's representable range;
    // from_timestamp_millis returns None → "—" fallback.
    assert_eq!(format_next_fire_ms(Some(i64::MAX), base_now()), "—");
}

#[test]
fn ms_helper_i64_min_returns_dash() {
    assert_eq!(format_next_fire_ms(Some(i64::MIN), base_now()), "—");
}
