use chrono::{TimeZone, Timelike, Utc};
use loopal_scheduler::{CronExpression, CronParseError};

#[test]
fn parse_valid_every_5_minutes() {
    let expr = CronExpression::parse("*/5 * * * *").unwrap();
    let now = Utc::now();
    let next = expr.next_after(&now);
    assert!(next.is_some());
    assert!(next.unwrap() > now);
}

#[test]
fn parse_valid_daily_at_9am() {
    let expr = CronExpression::parse("0 9 * * *").unwrap();
    assert_eq!(expr.as_str(), "0 9 * * *");
}

#[test]
fn parse_valid_weekdays_at_9am() {
    let expr = CronExpression::parse("0 9 * * 1-5").unwrap();
    let now = Utc::now();
    let next = expr.next_after(&now);
    assert!(next.is_some());
}

#[test]
fn reject_too_few_fields() {
    let err = CronExpression::parse("*/5 * *").unwrap_err();
    assert!(err.to_string().contains("5 fields"));
}

#[test]
fn reject_too_many_fields() {
    let err = CronExpression::parse("0 */5 * * * *").unwrap_err();
    assert!(err.to_string().contains("5 fields"));
}

#[test]
fn reject_invalid_syntax() {
    let err = CronExpression::parse("abc * * * *").unwrap_err();
    assert!(err.to_string().contains("invalid cron"));
}

#[test]
fn next_after_returns_future_time() {
    let expr = CronExpression::parse("* * * * *").unwrap(); // every minute
    let now = Utc::now();
    let next = expr.next_after(&now).unwrap();
    assert!(next > now);
}

#[test]
fn parse_at_with_fixed_time() {
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let expr = CronExpression::parse_at("30 10 * * *", now).unwrap();
    let next = expr.next_after(&now).unwrap();
    assert_eq!(next.minute(), 30);
    assert_eq!(next.hour(), 10);
}

#[test]
fn parse_at_rejects_never_firing_expression() {
    // February 30 never exists — the expression has no future occurrence ever.
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let err = CronExpression::parse_at("0 0 30 2 *", now).unwrap_err();
    assert_eq!(err, CronParseError::NoOccurrence);
    assert_eq!(err.to_string(), "cron expression will never fire");
}

#[test]
fn parse_at_accepts_weekly_past_this_weeks_fire() {
    // Monday 2026-03-30 10:00 — `0 9 * * 1` next fires the following
    // Monday 09:00, ~6d 23h away. The old 3-day lifetime gate would
    // reject this; the new rule accepts.
    let monday_after_fire = Utc.with_ymd_and_hms(2026, 3, 30, 10, 0, 0).unwrap();
    let expr = CronExpression::parse_at("0 9 * * 1", monday_after_fire).unwrap();
    let next = expr.next_after(&monday_after_fire).unwrap();
    let delta = next - monday_after_fire;
    // Must be strictly more than 3 days — proves the lifetime cap is gone.
    assert!(
        delta > chrono::Duration::days(3),
        "delta was {delta:?}, next={next}"
    );
    assert!(delta <= chrono::Duration::days(7));
}

#[test]
fn display_matches_as_str() {
    let expr = CronExpression::parse("*/10 * * * *").unwrap();
    assert_eq!(format!("{expr}"), expr.as_str());
}
