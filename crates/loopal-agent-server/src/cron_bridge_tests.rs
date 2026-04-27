//! Unit tests for `CronIdentity` — the diff-skip identity used by cron_bridge.

use super::*;

fn snap(id: &str, prompt: &str, recurring: bool, next_ms: Option<i64>) -> CronJobSnapshot {
    CronJobSnapshot {
        id: id.into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: prompt.into(),
        recurring,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: next_ms,
        durable: false,
    }
}

#[test]
fn identity_ignores_next_fire() {
    let a = snap("x", "prompt", true, Some(100));
    let b = snap("x", "prompt", true, Some(999_999));
    assert_eq!(CronIdentity::from(&a), CronIdentity::from(&b));
}

#[test]
fn identity_ignores_created_at() {
    let mut a = snap("x", "p", true, None);
    let mut b = a.clone();
    a.created_at_unix_ms = 1;
    b.created_at_unix_ms = 2;
    assert_eq!(CronIdentity::from(&a), CronIdentity::from(&b));
}

#[test]
fn identity_differs_on_id() {
    let a = snap("a", "p", true, None);
    let b = snap("b", "p", true, None);
    assert_ne!(CronIdentity::from(&a), CronIdentity::from(&b));
}

#[test]
fn identity_differs_on_prompt() {
    let a = snap("x", "p1", true, None);
    let b = snap("x", "p2", true, None);
    assert_ne!(CronIdentity::from(&a), CronIdentity::from(&b));
}

#[test]
fn identity_differs_on_recurring() {
    let a = snap("x", "p", true, None);
    let b = snap("x", "p", false, None);
    assert_ne!(CronIdentity::from(&a), CronIdentity::from(&b));
}

#[test]
fn identity_differs_on_cron_expr() {
    let mut a = snap("x", "p", true, None);
    let mut b = a.clone();
    a.cron_expr = "*/5 * * * *".into();
    b.cron_expr = "*/10 * * * *".into();
    assert_ne!(CronIdentity::from(&a), CronIdentity::from(&b));
}

#[test]
fn to_identity_set_is_order_insensitive() {
    let a = vec![snap("a", "p", true, None), snap("b", "p", true, None)];
    let b = vec![snap("b", "p", true, None), snap("a", "p", true, None)];
    assert_eq!(to_identity_set(&a), to_identity_set(&b));
}

#[test]
fn to_identity_set_dedups_same_identity() {
    let dupes = vec![snap("x", "p", true, None), snap("x", "p", true, None)];
    assert_eq!(to_identity_set(&dupes).len(), 1);
}

#[test]
fn to_snapshot_maps_none_next_fire() {
    use chrono::{DateTime, Utc};
    use loopal_scheduler::CronJobInfo;
    let now: DateTime<Utc> = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let info = CronJobInfo {
        id: "zzz".into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: "no next fire".into(),
        recurring: false,
        created_at: now,
        next_fire: None,
        durable: false,
    };
    let snap = super::to_snapshot_for_test(info);
    assert_eq!(snap.id, "zzz");
    assert!(
        snap.next_fire_unix_ms.is_none(),
        "None next_fire must map to Option::None"
    );
}

#[test]
fn to_snapshot_strips_newlines_from_prompt() {
    use chrono::{DateTime, Utc};
    use loopal_scheduler::CronJobInfo;
    let now: DateTime<Utc> = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let info = CronJobInfo {
        id: "nl".into(),
        cron_expr: "* * * * *".into(),
        prompt: "line1\nline2\rline3".into(),
        recurring: true,
        created_at: now,
        next_fire: Some(now),
        durable: false,
    };
    let snap = super::to_snapshot_for_test(info);
    assert!(!snap.prompt.contains('\n'));
    assert!(!snap.prompt.contains('\r'));
}
