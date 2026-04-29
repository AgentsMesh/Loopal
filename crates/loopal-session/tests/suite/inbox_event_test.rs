use loopal_protocol::{AgentEvent, AgentEventPayload, MessageSource, QualifiedAddress};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

fn apply_inbox(state: &mut SessionState, agent: &str, source: MessageSource, content: &str) {
    apply_event(
        state,
        AgentEvent::named(
            agent,
            AgentEventPayload::InboxEnqueued {
                message_id: "m-1".into(),
                source,
                content: content.into(),
                summary: None,
            },
        ),
    );
}

#[test]
fn test_inbox_enqueued_from_human_skips_conversation_push() {
    let mut state = make_state();
    apply_inbox(&mut state, "main", MessageSource::Human, "hi from user");
    let conv = &state.agents["main"].conversation;
    assert!(
        conv.messages.is_empty(),
        "Human source already rendered by append_user_display"
    );
}

#[test]
fn test_inbox_enqueued_from_agent_carries_qualified_source() {
    let mut state = make_state();
    let src = MessageSource::Agent(QualifiedAddress::local("worker"));
    apply_inbox(&mut state, "main", src.clone(), "ping");
    let origin = state.agents["main"].conversation.messages[0]
        .inbox
        .as_ref()
        .unwrap();
    assert_eq!(origin.source, src);
}

#[test]
fn test_inbox_enqueued_from_scheduled_marks_origin() {
    let mut state = make_state();
    apply_inbox(&mut state, "main", MessageSource::Scheduled, "tick");
    let origin = state.agents["main"].conversation.messages[0]
        .inbox
        .as_ref()
        .unwrap();
    assert_eq!(origin.source, MessageSource::Scheduled);
}

#[test]
fn test_inbox_enqueued_from_channel_records_channel_metadata() {
    let mut state = make_state();
    let src = MessageSource::Channel {
        channel: "general".into(),
        from: QualifiedAddress::local("bot"),
    };
    apply_inbox(&mut state, "main", src.clone(), "broadcast");
    assert_eq!(
        state.agents["main"].conversation.messages[0]
            .inbox
            .as_ref()
            .unwrap()
            .source,
        src
    );
}

#[test]
fn test_inbox_enqueued_from_system_records_kind() {
    let mut state = make_state();
    let src = MessageSource::System("rewake".into());
    apply_inbox(&mut state, "main", src.clone(), "hook signal");
    assert_eq!(
        state.agents["main"].conversation.messages[0]
            .inbox
            .as_ref()
            .unwrap()
            .source,
        src
    );
}

#[test]
fn test_inbox_enqueued_summary_propagates_to_message() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named(
            "main",
            AgentEventPayload::InboxEnqueued {
                message_id: "m".into(),
                source: MessageSource::Agent(QualifiedAddress::local("a")),
                content: "very long content body...".into(),
                summary: Some("ping".into()),
            },
        ),
    );
    let origin = state.agents["main"].conversation.messages[0]
        .inbox
        .as_ref()
        .unwrap();
    assert_eq!(origin.summary.as_deref(), Some("ping"));
}

#[test]
fn test_inbox_consumed_does_not_alter_conversation() {
    let mut state = make_state();
    apply_inbox(&mut state, "main", MessageSource::Scheduled, "first");
    apply_event(
        &mut state,
        AgentEvent::named(
            "main",
            AgentEventPayload::InboxConsumed {
                message_id: "m-1".into(),
            },
        ),
    );
    assert_eq!(state.agents["main"].conversation.messages.len(), 1);
}

#[test]
fn test_inbox_enqueued_routes_per_agent_name() {
    let mut state = make_state();
    apply_inbox(&mut state, "worker-a", MessageSource::Scheduled, "to a");
    apply_inbox(&mut state, "worker-b", MessageSource::Scheduled, "to b");
    assert_eq!(
        state.agents["worker-a"].conversation.messages[0].content,
        "to a"
    );
    assert_eq!(
        state.agents["worker-b"].conversation.messages[0].content,
        "to b"
    );
}

#[test]
fn test_human_optimistic_display_plus_inbox_event_emits_single_row() {
    let mut state = make_state();
    state
        .agents
        .entry("main".to_string())
        .or_default()
        .conversation
        .messages
        .push(loopal_session::SessionMessage {
            role: "user".into(),
            content: "hi".into(),
            ..Default::default()
        });

    apply_inbox(&mut state, "main", MessageSource::Human, "hi");

    let msgs = &state.agents["main"].conversation.messages;
    assert_eq!(
        msgs.len(),
        1,
        "Human optimistic display + InboxEnqueued{{Human}} must not duplicate"
    );
    assert!(msgs[0].inbox.is_none());
}
