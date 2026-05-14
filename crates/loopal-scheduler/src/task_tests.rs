use chrono::{TimeZone, Timelike, Utc};

use crate::expression::CronExpression;
use crate::task::{ScheduledTask, truncate_to_secs};

fn make_task(cron: &str, created_at: chrono::DateTime<Utc>) -> ScheduledTask {
    ScheduledTask {
        id: "test1234".into(),
        cron: CronExpression::parse_at(cron, created_at).unwrap(),
        prompt: "test".into(),
        recurring: true,
        created_at,
        last_fired: None,
        durable: false,
    }
}

#[test]
fn truncate_removes_nanoseconds() {
    let dt = Utc.with_ymd_and_hms(2026, 3, 29, 10, 5, 30).unwrap()
        + chrono::Duration::nanoseconds(123_456_789);
    let truncated = truncate_to_secs(dt);
    assert_eq!(truncated.nanosecond(), 0);
    assert_eq!(truncated.second(), 30);
}

#[test]
fn truncate_zero_nanoseconds_is_identity() {
    let dt = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    assert_eq!(truncate_to_secs(dt), dt);
}

#[test]
fn should_fire_true_when_next_occurrence_is_past() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let task = make_task("* * * * *", created); // every minute
    // At 10:01:05, next_after(10:00:00) = 10:01:00 <= 10:01:05 → true
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 5).unwrap();
    assert!(task.should_fire(&now));
}

#[test]
fn should_fire_false_before_next_occurrence() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let task = make_task("* * * * *", created);
    // At 10:00:30, next_after(10:00:00) = 10:01:00 > 10:00:30 → false
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 30).unwrap();
    assert!(!task.should_fire(&now));
}

#[test]
fn should_fire_uses_last_fired() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let mut task = make_task("* * * * *", created);
    task.last_fired = Some(Utc.with_ymd_and_hms(2026, 3, 29, 10, 5, 0).unwrap());
    // At 10:06:05, next_after(10:05:00) = 10:06:00 <= 10:06:05 → true
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 6, 5).unwrap();
    assert!(task.should_fire(&now));
}

#[test]
fn should_fire_no_double_fire_same_minute() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let mut task = make_task("* * * * *", created);
    // Fired at 10:01:00, check again at 10:01:30 — should not re-fire.
    task.last_fired = Some(Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 0).unwrap());
    let now = Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 30).unwrap();
    assert!(!task.should_fire(&now));
}

#[test]
fn next_fire_starts_from_created_at_when_not_fired() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let task = make_task("0 12 * * *", created);
    // Daily at 12:00 — next after 10:00 created_at is same day 12:00.
    let expected = Utc.with_ymd_and_hms(2026, 3, 29, 12, 0, 0).unwrap();
    assert_eq!(task.next_fire(), Some(expected));
}

#[test]
fn next_fire_advances_after_last_fired() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let mut task = make_task("0 12 * * *", created);
    task.last_fired = Some(Utc.with_ymd_and_hms(2026, 3, 29, 12, 0, 0).unwrap());
    // Already fired today's 12:00 — next is tomorrow's 12:00.
    let expected = Utc.with_ymd_and_hms(2026, 3, 30, 12, 0, 0).unwrap();
    assert_eq!(task.next_fire(), Some(expected));
}

#[test]
fn next_fire_truncates_sub_second_reference() {
    let created = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 30).unwrap()
        + chrono::Duration::nanoseconds(999_999);
    let task = make_task("* * * * *", created);
    // Truncated reference 10:00:30 → next at 10:01:00.
    let expected = Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 0).unwrap();
    assert_eq!(task.next_fire(), Some(expected));
}
