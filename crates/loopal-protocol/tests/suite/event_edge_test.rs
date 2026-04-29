use loopal_protocol::{AgentEvent, AgentEventPayload, MessageSource, QualifiedAddress};

#[test]
fn test_event_message_routed_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: MessageSource::Agent(QualifiedAddress::local("agent-a")),
        target: QualifiedAddress::local("agent-b"),
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
        assert_eq!(
            source,
            MessageSource::Agent(QualifiedAddress::local("agent-a"))
        );
        assert_eq!(target, QualifiedAddress::local("agent-b"));
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
    assert_eq!(
        deserialized.agent_name,
        Some(QualifiedAddress::local("worker"))
    );
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

#[test]
fn test_event_rewound_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::Rewound { remaining_turns: 3 });
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::Rewound { remaining_turns } = deserialized.payload {
        assert_eq!(remaining_turns, 3);
    } else {
        panic!("expected AgentEventPayload::Rewound");
    }
}

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
fn test_event_inbox_enqueued_serde_roundtrip() {
    let event = AgentEvent::named(
        "main",
        AgentEventPayload::InboxEnqueued {
            message_id: "msg-001".into(),
            source: MessageSource::Agent(QualifiedAddress::local("worker")),
            content: "the full body, possibly long".into(),
            summary: Some("ping".into()),
        },
    );
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    let AgentEventPayload::InboxEnqueued {
        message_id,
        source,
        content,
        summary,
    } = de.payload
    else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(message_id, "msg-001");
    assert_eq!(
        source,
        MessageSource::Agent(QualifiedAddress::local("worker"))
    );
    assert_eq!(content, "the full body, possibly long");
    assert_eq!(summary.as_deref(), Some("ping"));
}

#[test]
fn test_event_inbox_enqueued_summary_omitted_when_none() {
    let event = AgentEvent::root(AgentEventPayload::InboxEnqueued {
        message_id: "m".into(),
        source: MessageSource::Human,
        content: "hi".into(),
        summary: None,
    });
    let json = serde_json::to_string(&event).unwrap();
    assert!(!json.contains("\"summary\""), "json was: {json}");
}

#[test]
fn test_event_inbox_consumed_serde_roundtrip() {
    let event = AgentEvent::root(AgentEventPayload::InboxConsumed {
        message_id: "msg-7".into(),
    });
    let json = serde_json::to_string(&event).unwrap();
    let de: AgentEvent = serde_json::from_str(&json).unwrap();
    if let AgentEventPayload::InboxConsumed { message_id } = de.payload {
        assert_eq!(message_id, "msg-7");
    } else {
        panic!("expected InboxConsumed");
    }
}

#[test]
fn test_inbox_enqueued_snat_promotes_local_agent_source() {
    use loopal_protocol::address::QualifiedAddress;
    let mut payload = AgentEventPayload::InboxEnqueued {
        message_id: "m".into(),
        source: MessageSource::Agent(QualifiedAddress::local("worker")),
        content: "body".into(),
        summary: None,
    };
    payload.prepend_self_hub("hub-A");
    let AgentEventPayload::InboxEnqueued { source, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(
        source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-A"], "worker"))
    );
}

#[test]
fn test_inbox_enqueued_snat_noop_for_non_addressable_source() {
    let mut payload = AgentEventPayload::InboxEnqueued {
        message_id: "m".into(),
        source: MessageSource::Scheduled,
        content: "tick".into(),
        summary: None,
    };
    payload.prepend_self_hub("hub-A");
    let AgentEventPayload::InboxEnqueued { source, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(source, MessageSource::Scheduled);
}
