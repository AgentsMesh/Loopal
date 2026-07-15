use loopal_protocol::{AgentEvent, AgentEventPayload, ProjectedMessage, SessionHistorySnapshot};

#[test]
fn test_event_session_resumed_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::SessionResumed {
        session_id: "abc-123".into(),
        message_count: 42,
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::SessionResumed {
        session_id,
        message_count,
    } = de.payload
    {
        assert_eq!(session_id, "abc-123");
        assert_eq!(message_count, 42);
    } else {
        panic!("expected AgentEventPayload::SessionResumed");
    }
}

#[test]
fn test_event_session_resume_warnings_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::SessionResumeWarnings {
        session_id: "xyz-789".into(),
        warnings: vec!["cron failed: io".into(), "task failed: locked".into()],
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::SessionResumeWarnings {
        session_id,
        warnings,
    } = de.payload
    {
        assert_eq!(session_id, "xyz-789");
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("cron"));
    } else {
        panic!("expected AgentEventPayload::SessionResumeWarnings");
    }
}

#[test]
fn test_event_session_resume_warnings_empty_warnings_roundtrips() {
    let event = AgentEvent::root(AgentEventPayload::SessionResumeWarnings {
        session_id: "id".into(),
        warnings: Vec::new(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::SessionResumeWarnings { warnings, .. } = de.payload {
        assert!(warnings.is_empty());
    } else {
        panic!("expected SessionResumeWarnings");
    }
}

#[test]
fn session_history_loaded_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::SessionHistoryLoaded(
        SessionHistorySnapshot {
            session_id: "session-1".into(),
            messages: vec![ProjectedMessage {
                role: "user".into(),
                content: "persisted".into(),
                tool_calls: vec![],
                image_count: 1,
                skill_info: None,
            }],
            truncated: true,
        },
    ));
    let decoded: AgentEvent =
        serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
    let AgentEventPayload::SessionHistoryLoaded(history) = decoded.payload else {
        panic!("expected SessionHistoryLoaded");
    };
    assert_eq!(history.session_id, "session-1");
    assert_eq!(history.messages[0].content, "persisted");
    assert_eq!(history.messages[0].image_count, 1);
    assert!(history.truncated);
}
