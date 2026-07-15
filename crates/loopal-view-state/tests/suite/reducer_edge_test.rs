use loopal_protocol::{AgentEventPayload, MessageSource, QualifiedAddress, SkillInvocation};
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

#[test]
fn human_inbox_is_retained_and_deduplicated_in_either_event_order() {
    let inbox = || AgentEventPayload::InboxEnqueued {
        envelope_id: "prompt".into(),
        source: MessageSource::Human,
        content: "headless child prompt".into(),
        summary: None,
    };
    let queued = || AgentEventPayload::UserMessageQueued {
        envelope_id: "prompt".into(),
        content: "headless child prompt".into(),
        image_count: 0,
        skill_info: None,
    };

    let mut inbox_first = ViewStateReducer::new("child");
    assert!(inbox_first.apply(inbox()).is_some());
    assert!(inbox_first.apply(queued()).is_none());
    assert_eq!(inbox_first.state().agent.conversation.messages.len(), 1);

    let mut queued_first = ViewStateReducer::new("main");
    assert!(queued_first.apply(queued()).is_some());
    assert!(queued_first.apply(inbox()).is_none());
    assert_eq!(queued_first.state().agent.conversation.messages.len(), 1);
    assert_eq!(
        inbox_first.state().agent.conversation.messages[0].content,
        "headless child prompt"
    );
}

#[test]
fn internal_goal_continuation_is_not_rendered_as_a_user_message() {
    let mut reducer = ViewStateReducer::new("main");
    let result = reducer.apply(AgentEventPayload::InboxEnqueued {
        envelope_id: "goal-continuation".into(),
        source: MessageSource::System("goal_continuation".into()),
        content: "Continue working toward the active thread goal".into(),
        summary: None,
    });
    assert!(result.is_none());
    assert!(reducer.state().agent.conversation.messages.is_empty());
}

#[test]
fn queued_skill_metadata_reaches_the_conversation() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(AgentEventPayload::UserMessageQueued {
        envelope_id: "skill".into(),
        content: "expanded prompt".into(),
        image_count: 0,
        skill_info: Some(SkillInvocation {
            name: "/desktop-check".into(),
            user_args: "alpha beta".into(),
        }),
    });
    let message = &reducer.state().agent.conversation.messages[0];
    assert_eq!(message.skill_info.as_ref().unwrap().name, "/desktop-check");
    assert_eq!(message.skill_info.as_ref().unwrap().user_args, "alpha beta");
}
