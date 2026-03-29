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
fn parse_at_rejects_no_occurrence_within_lifetime() {
    // Use February 30 which never exists — no valid occurrence ever.
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let err = CronExpression::parse_at("0 0 30 2 *", now).unwrap_err();
    assert_eq!(err, CronParseError::NoOccurrence);
}

#[test]
fn display_matches_as_str() {
    let expr = CronExpression::parse("*/10 * * * *").unwrap();
    assert_eq!(format!("{expr}"), expr.as_str());
}
