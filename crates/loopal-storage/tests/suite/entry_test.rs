use loopal_message::Message;
use loopal_storage::entry::{Marker, TaggedEntry};

#[test]
fn tagged_message_roundtrip() {
    let msg = Message::user("hello");
    let entry = TaggedEntry::Message(msg);
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"_type\":\"message\""));

    let decoded: TaggedEntry = serde_json::from_str(&json).unwrap();
    match decoded {
        TaggedEntry::Message(m) => assert_eq!(m.text_content(), "hello"),
        other => panic!("expected Message, got {other:?}"),
    }
}

#[test]
fn clear_marker_roundtrip() {
    let entry = TaggedEntry::Marker(Marker::Clear {
        timestamp: "2025-01-01T00:00:00Z".into(),
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"_type\":\"marker\""));
    assert!(json.contains("\"kind\":\"clear\""));

    let decoded: TaggedEntry = serde_json::from_str(&json).unwrap();
    match decoded {
        TaggedEntry::Marker(Marker::Clear { timestamp }) => {
            assert_eq!(timestamp, "2025-01-01T00:00:00Z");
        }
        other => panic!("expected Clear marker, got {other:?}"),
    }
}

#[test]
fn compact_boundary_marker_roundtrip() {
    let entry = TaggedEntry::Marker(Marker::CompactBoundary {
        summary_msg_id: "summary-msg-001".into(),
        timestamp: "2025-06-15T12:00:00Z".into(),
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"compact_boundary\""));
    assert!(json.contains("\"summary-msg-001\""));

    let decoded: TaggedEntry = serde_json::from_str(&json).unwrap();
    match decoded {
        TaggedEntry::Marker(Marker::CompactBoundary {
            summary_msg_id,
            timestamp,
        }) => {
            assert_eq!(summary_msg_id, "summary-msg-001");
            assert_eq!(timestamp, "2025-06-15T12:00:00Z");
        }
        other => panic!("expected CompactBoundary marker, got {other:?}"),
    }
}

#[test]
fn old_compact_to_marker_fails_to_decode() {
    let json = r#"{"_type":"marker","kind":"compact_to","keep_last":5,"timestamp":"x"}"#;
    assert!(
        serde_json::from_str::<TaggedEntry>(json).is_err(),
        "legacy compact_to marker must be rejected after removal"
    );
}

#[test]
fn message_with_id_roundtrip() {
    let msg = Message::user("with id").with_id("msg-001".into());
    let entry = TaggedEntry::Message(msg);
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"id\":\"msg-001\""));

    let decoded: TaggedEntry = serde_json::from_str(&json).unwrap();
    if let TaggedEntry::Message(m) = decoded {
        assert_eq!(m.id.as_deref(), Some("msg-001"));
    } else {
        panic!("expected Message");
    }
}

#[test]
fn message_without_id_omits_field() {
    let msg = Message::user("no id");
    let entry = TaggedEntry::Message(msg);
    let json = serde_json::to_string(&entry).unwrap();
    // id should be skipped when None (skip_serializing_if on Message)
    assert!(!json.contains("\"id\""), "id should be skipped: {json}");
}

#[test]
fn rewind_to_marker_roundtrip() {
    let entry = TaggedEntry::Marker(Marker::RewindTo {
        message_id: "msg-042".into(),
        timestamp: "2025-08-01T10:00:00Z".into(),
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"rewind_to\""));
    assert!(json.contains("\"msg-042\""));

    let decoded: TaggedEntry = serde_json::from_str(&json).unwrap();
    match decoded {
        TaggedEntry::Marker(Marker::RewindTo {
            message_id,
            timestamp,
        }) => {
            assert_eq!(message_id, "msg-042");
            assert_eq!(timestamp, "2025-08-01T10:00:00Z");
        }
        other => panic!("expected RewindTo marker, got {other:?}"),
    }
}
