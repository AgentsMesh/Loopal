use loopal_protocol::AgentEventPayload;
use loopal_view_state::ViewStateReducer;

#[test]
fn apply_with_rev_changes_revision_only_for_mutations() {
    let mut reducer = ViewStateReducer::new("main");
    assert_eq!(
        reducer.apply_with_rev(AgentEventPayload::Running, 42),
        Some(42)
    );
    assert_eq!(
        reducer.apply_with_rev(
            AgentEventPayload::TurnDiffSummary {
                modified_files: vec![],
            },
            99,
        ),
        None
    );
    assert_eq!(reducer.rev(), 42);
}

#[test]
fn reset_to_restores_state_and_revision() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(AgentEventPayload::Stream {
        text: "kept".into(),
    });
    let snapshot = reducer.snapshot();
    reducer.apply(AgentEventPayload::Stream {
        text: " discarded".into(),
    });

    reducer.reset_to(snapshot);
    assert_eq!(reducer.rev(), 1);
    assert_eq!(reducer.state().agent.conversation.streaming_text, "kept");
}

#[test]
fn mutable_accessors_do_not_change_revision() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.with_conversation_mut(|conversation| {
        conversation.streaming_text = "manual".into();
    });
    reducer.with_view_mut(|view| {
        view.parent = Some("parent".into());
    });

    assert_eq!(reducer.rev(), 0);
    assert_eq!(reducer.state().agent.conversation.streaming_text, "manual");
    assert_eq!(reducer.state().agent.parent.as_deref(), Some("parent"));
}
