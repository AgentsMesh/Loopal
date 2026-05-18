use loopal_protocol::{TaskSnapshot, TaskSnapshotStatus};

fn sample_snapshot() -> TaskSnapshot {
    TaskSnapshot {
        id: "1".into(),
        subject: "Fix the bug".into(),
        active_form: Some("Fixing the bug".into()),
        status: TaskSnapshotStatus::InProgress,
        blocked_by: vec!["2".into()],
        description: "Investigate the crash reported in #42".into(),
        blocks: vec!["3".into(), "4".into()],
    }
}

#[test]
fn serde_roundtrip_preserves_all_fields() {
    let snap = sample_snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let back: TaskSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, snap.id);
    assert_eq!(back.subject, snap.subject);
    assert_eq!(back.description, snap.description);
    assert_eq!(back.blocks, snap.blocks);
    assert_eq!(back.blocked_by, snap.blocked_by);
}

#[test]
fn missing_new_fields_deserialize_with_defaults() {
    let legacy = r#"{
        "id":"1",
        "subject":"old task",
        "active_form":null,
        "status":"pending",
        "blocked_by":[]
    }"#;
    let back: TaskSnapshot = serde_json::from_str(legacy).unwrap();
    assert_eq!(back.id, "1");
    assert_eq!(back.description, "");
    assert!(back.blocks.is_empty());
}

#[test]
fn pending_status_with_blocks_serde() {
    let snap = TaskSnapshot {
        id: "9".into(),
        subject: "Wait".into(),
        active_form: None,
        status: TaskSnapshotStatus::Pending,
        blocked_by: vec!["1".into()],
        description: String::new(),
        blocks: Vec::new(),
    };
    let json = serde_json::to_string(&snap).unwrap();
    let back: TaskSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.status, TaskSnapshotStatus::Pending);
    assert_eq!(back.blocked_by, vec!["1".to_string()]);
}
