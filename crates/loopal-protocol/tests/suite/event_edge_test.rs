use loopal_protocol::{AgentEvent, AgentEventPayload};

#[test]
fn test_event_message_routed_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: "agent-a".into(),
        target: "agent-b".into(),
        content_preview: "hello world".into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::MessageRouted {
        source,
        target,
        content_preview,
    } = deserialized.payload
    {
        assert_eq!(source, "agent-a");
        assert_eq!(target, "agent-b");
        assert_eq!(content_preview, "hello world");
    } else {
        panic!("expected AgentEventPayload::MessageRouted");
    }
}

#[test]
fn test_event_named_agent_serde_roundtrip() {
    let event = AgentEvent::named("worker", AgentEventPayload::Started);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.agent_name, Some("worker".to_string()));
    assert!(matches!(deserialized.payload, AgentEventPayload::Started));
}

#[test]
fn test_event_root_agent_name_is_none() {
    let event = AgentEvent::root(AgentEventPayload::Started);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(deserialized.agent_name.is_none());
}

#[test]
fn test_event_retry_error_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::RetryError {
        message: "502 Bad Gateway. Retrying in 2.0s".into(),
        attempt: 1,
        max_attempts: 6,
    });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::RetryError {
        message,
        attempt,
        max_attempts,
    } = deserialized.payload
    {
        assert_eq!(message, "502 Bad Gateway. Retrying in 2.0s");
        assert_eq!(attempt, 1);
        assert_eq!(max_attempts, 6);
    } else {
        panic!("expected AgentEventPayload::RetryError");
    }
}

#[test]
fn test_event_retry_cleared_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::RetryCleared);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(
        deserialized.payload,
        AgentEventPayload::RetryCleared
    ));
}
