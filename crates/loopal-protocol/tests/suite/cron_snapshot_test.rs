use loopal_protocol::{AgentEventPayload, CronJobSnapshot};

fn sample_snapshot() -> CronJobSnapshot {
    CronJobSnapshot {
        id: "abc12345".into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: "run daily cleanup".into(),
        recurring: true,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: Some(1_700_000_000_000),
    }
}

#[test]
fn serde_roundtrip_preserves_all_fields() {
    let snap = sample_snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let back: CronJobSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back, snap);
}

#[test]
fn serde_roundtrip_with_none_next_fire() {
    let snap = CronJobSnapshot {
        next_fire_unix_ms: None,
        ..sample_snapshot()
    };
    let json = serde_json::to_string(&snap).unwrap();
    let back: CronJobSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.next_fire_unix_ms, None);
}

#[test]
fn partial_eq_differs_on_next_fire() {
    let a = sample_snapshot();
    let b = CronJobSnapshot {
        next_fire_unix_ms: Some(1_700_000_001_000),
        ..sample_snapshot()
    };
    assert_ne!(a, b);
}

#[test]
fn partial_eq_differs_on_recurring() {
    let a = sample_snapshot();
    let b = CronJobSnapshot {
        recurring: false,
        ..sample_snapshot()
    };
    assert_ne!(a, b);
}

#[test]
fn debug_format_contains_id() {
    let snap = sample_snapshot();
    let debug = format!("{snap:?}");
    assert!(debug.contains("abc12345"));
}

#[test]
fn crons_changed_event_serde() {
    let event = AgentEventPayload::CronsChanged {
        crons: vec![sample_snapshot()],
    };
    let json = serde_json::to_string(&event).unwrap();
    let back: AgentEventPayload = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::CronsChanged { crons } = back {
        assert_eq!(crons.len(), 1);
        assert_eq!(crons[0].id, "abc12345");
        assert_eq!(crons[0].prompt, "run daily cleanup");
        assert!(crons[0].recurring);
    } else {
        panic!("expected CronsChanged");
    }
}

#[test]
fn crons_changed_event_empty_list() {
    let event = AgentEventPayload::CronsChanged { crons: vec![] };
    let json = serde_json::to_string(&event).unwrap();
    let back: AgentEventPayload = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::CronsChanged { crons } = back {
        assert!(crons.is_empty());
    } else {
        panic!("expected CronsChanged");
    }
}

#[test]
fn new_fields_roundtrip_preserves_cron_expr_and_created_at() {
    let snap = sample_snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let back: CronJobSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.cron_expr, "*/5 * * * *");
    assert_eq!(back.created_at_unix_ms, 1_700_000_000_000);
}

#[test]
fn missing_new_fields_deserialize_with_defaults() {
    // Compatibility check: older payloads without cron_expr / created_at
    // should deserialize using serde(default).
    let legacy = r#"{
        "id":"old",
        "prompt":"legacy",
        "recurring":true,
        "next_fire_unix_ms":null
    }"#;
    let back: CronJobSnapshot = serde_json::from_str(legacy).unwrap();
    assert_eq!(back.id, "old");
    assert_eq!(back.cron_expr, "");
    assert_eq!(back.created_at_unix_ms, 0);
    assert!(back.next_fire_unix_ms.is_none());
}
