//! Constructor + state-accessor tests for `ViewStateReducer`.
//!
//! Pins down: rev starts at 0/1 depending on entry path, snapshot
//! mirrors current rev, state accessor reflects mutations.

use loopal_protocol::{AgentEventPayload, AgentStateSnapshot, TaskSnapshot, TaskSnapshotStatus};
use loopal_view_state::ViewStateReducer;

#[test]
fn new_starts_with_rev_zero() {
    let r = ViewStateReducer::new("root");
    assert_eq!(r.rev(), 0);
    assert_eq!(r.state().agent.name, "root");
    assert!(r.state().tasks.is_empty());
}

#[test]
fn from_snapshot_starts_with_rev_one() {
    let snap = AgentStateSnapshot {
        tasks: vec![TaskSnapshot {
            id: "t1".into(),
            subject: "preserved".into(),
            active_form: None,
            status: TaskSnapshotStatus::Pending,
            blocked_by: vec![],
            description: String::new(),
            blocks: vec![],
        }],
        crons: vec![],
        bg_tasks: vec![],
        thread_goal: None,
        workflows: Default::default(),
    };
    let r = ViewStateReducer::from_snapshot("root", snap);
    assert_eq!(r.rev(), 1);
    assert_eq!(r.state().tasks.len(), 1);
    assert_eq!(r.state().tasks[0].subject, "preserved");
}

#[test]
fn snapshot_method_mirrors_current_rev_and_state() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);

    let snap = r.snapshot();
    assert_eq!(snap.rev, 1);
    assert_eq!(
        snap.state.agent.observable.status,
        loopal_protocol::AgentStatus::Running
    );
}

#[test]
fn rev_is_strictly_monotonic_across_observable_events() {
    let mut r = ViewStateReducer::new("root");
    let mut last_rev = 0;
    for evt in [
        AgentEventPayload::Started,
        AgentEventPayload::Running,
        AgentEventPayload::AwaitingInput,
        AgentEventPayload::Finished,
    ] {
        r.apply(evt);
        assert!(r.rev() > last_rev);
        last_rev = r.rev();
    }
    assert_eq!(r.rev(), 4);
}

#[test]
fn rev_unchanged_when_event_is_non_observable() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    let after_running = r.rev();

    let result = r.apply(AgentEventPayload::TurnDiffSummary {
        modified_files: vec![],
    });
    assert!(result.is_none());
    assert_eq!(r.rev(), after_running);
}

#[test]
fn cleared_event_resets_conversation_and_counters() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    r.apply(AgentEventPayload::Stream {
        text: "partial".into(),
    });
    r.apply(AgentEventPayload::ToolCall {
        id: "t1".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/x"}),
    });
    r.apply(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        thinking_tokens: 0,
    });

    r.apply(AgentEventPayload::Cleared {
        context_window: 200_000,
    });

    let state = r.state();
    assert!(state.agent.conversation.messages.is_empty());
    assert!(state.agent.conversation.streaming_text.is_empty());
    assert_eq!(state.agent.conversation.turn_count, 0);
    assert_eq!(state.agent.conversation.input_tokens, 0);
    assert_eq!(state.agent.conversation.output_tokens, 0);
    assert_eq!(state.agent.observable.input_tokens, 0);
    assert_eq!(state.agent.observable.output_tokens, 0);
    assert_eq!(state.agent.observable.turn_count, 0);
    assert_eq!(state.agent.tool_count(), 0);
    assert_eq!(state.agent.tools_in_flight(), 0);
    assert!(state.agent.last_tool().is_none());
}

#[test]
fn cleared_event_drops_pending_permission_to_prevent_zombie_dialog() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    r.apply(AgentEventPayload::ToolPermissionRequest {
        id: "perm-1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "rm -rf /"}),
        permission_intent: None,
    });
    assert!(
        r.state().agent.conversation.pending_permission.is_some(),
        "fixture should have a pending permission row to clear"
    );

    r.apply(AgentEventPayload::Cleared {
        context_window: 200_000,
    });

    assert!(
        r.state().agent.conversation.pending_permission.is_none(),
        "Cleared must drop the pending permission so the dialog cannot \
         outlive the message row it pointed at"
    );
}

#[test]
fn model_changed_event_updates_observable_model() {
    let mut r = ViewStateReducer::new("root");
    assert!(r.state().agent.observable.model.is_empty());
    r.apply(AgentEventPayload::ModelChanged {
        model: "claude-opus-4-7".into(),
    });
    assert_eq!(r.state().agent.observable.model, "claude-opus-4-7");
}

#[test]
fn thinking_changed_event_normalizes_json_to_label() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ThinkingChanged {
        thinking_config: r#"{"type":"effort","level":"high"}"#.into(),
    });
    assert_eq!(r.state().agent.observable.thinking_config, "high");
}

#[test]
fn thinking_changed_disabled_event_normalizes_to_disabled() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ThinkingChanged {
        thinking_config: r#"{"type":"disabled"}"#.into(),
    });
    assert_eq!(r.state().agent.observable.thinking_config, "disabled");
}

#[test]
fn thinking_changed_event_falls_back_to_auto_on_garbage() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ThinkingChanged {
        thinking_config: "not-json".into(),
    });
    assert_eq!(r.state().agent.observable.thinking_config, "auto");
}

#[test]
fn mode_changed_event_still_routes_through_lifecycle() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ModeChanged {
        mode: "plan".into(),
    });
    assert_eq!(r.state().agent.observable.mode, "plan");
}
