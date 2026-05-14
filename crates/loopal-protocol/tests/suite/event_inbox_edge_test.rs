use loopal_protocol::{AgentEvent, AgentEventPayload, MessageSource, QualifiedAddress};

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
