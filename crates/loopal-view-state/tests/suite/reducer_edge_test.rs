use loopal_protocol::{AgentEventPayload, MessageSource, QualifiedAddress};
use loopal_view_state::ViewStateReducer;

#[test]
fn message_routed_is_non_observable() {
    let mut r = ViewStateReducer::new("root");
    let result = r.apply(AgentEventPayload::MessageRouted {
        source: MessageSource::Human,
        target: QualifiedAddress::local("self"),
        content_preview: "hi".into(),
    });
    assert!(result.is_none());
}

#[test]
fn turn_diff_summary_is_non_observable() {
    let mut r = ViewStateReducer::new("root");
    let result = r.apply(AgentEventPayload::TurnDiffSummary {
        modified_files: vec![],
    });
    assert!(result.is_none());
}

#[test]
fn delta_chain_has_consecutive_revs() {
    let mut r = ViewStateReducer::new("root");
    let r1 = r.apply(AgentEventPayload::Running).expect("observable");
    let r2 = r
        .apply(AgentEventPayload::AwaitingInput)
        .expect("observable");
    assert_eq!(r2, r1 + 1);
}

#[test]
fn stream_events_now_mutate_conversation() {
    let mut r = ViewStateReducer::new("root");
    let new_rev = r
        .apply(AgentEventPayload::Stream {
            text: "hello".into(),
        })
        .expect("conversation mutation produces a rev bump");
    assert_eq!(new_rev, 1);
    assert_eq!(r.state().agent.conversation.streaming_text, "hello");
}
