use loopal_protocol::{AgentEventPayload, TurnSummary};
use loopal_tool_invocation::{InvocationState, StaleReason};
use loopal_view_state::ViewStateReducer;

fn turn_completed_payload() -> AgentEventPayload {
    AgentEventPayload::TurnCompleted(TurnSummary {
        turn_id: 1,
        duration_ms: 0,
        llm_calls: 0,
        tool_calls_requested: 0,
        tool_calls_approved: 0,
        tool_calls_denied: 0,
        tool_errors: 0,
        auto_continuations: 0,
        warnings_injected: 0,
        tokens_in: 0,
        tokens_out: 0,
        modified_files: Vec::new(),
    })
}

#[test]
fn running_invocation_marked_stale_on_turn_end() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 100"}),
    });
    r.apply(AgentEventPayload::ToolProgress {
        id: "tc-1".into(),
        name: "Bash".into(),
        output_tail: "".into(),
        elapsed_ms: 200,
    });

    let tc_before = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc_before.state, InvocationState::Running { .. }));

    r.apply(turn_completed_payload());

    let tc_after = &r.state().agent.conversation.messages[0].tool_calls[0];
    let InvocationState::Stale { reason, .. } = &tc_after.state else {
        panic!("expected Stale, got {:?}", tc_after.state.variant_name())
    };
    assert_eq!(*reason, StaleReason::TurnEnded);
}

#[test]
fn done_invocation_unaffected_by_turn_end() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-2".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/tmp/x"}),
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "tc-2".into(),
        name: "Read".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: Some(10),
        metadata: None,
    });

    r.apply(turn_completed_payload());

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Done { .. }));
}

#[test]
fn pending_invocation_marked_stale_too() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-3".into(),
        name: "Edit".into(),
        input: serde_json::json!({"file_path": "/tmp/y"}),
    });

    let tc_before = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc_before.state, InvocationState::Pending));

    r.apply(turn_completed_payload());

    let tc_after = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc_after.state, InvocationState::Stale { .. }));
}

#[test]
fn turn_end_reconcile_resets_tools_in_flight() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-a".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 99"}),
    });
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-b".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 88"}),
    });
    assert_eq!(r.state().agent.tools_in_flight(), 2);
    assert!(r.state().agent.last_tool().is_some());

    r.apply(turn_completed_payload());

    let obs = &r.state().agent; /* observable now lifted to AgentView methods */
    assert_eq!(
        obs.tools_in_flight(),
        0,
        "turn-end reconcile must zero tools_in_flight"
    );
    assert!(
        obs.last_tool().is_none(),
        "turn-end reconcile must clear last_tool"
    );
}

#[test]
fn turn_end_reconcile_preserves_in_flight_when_no_active_tools() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-x".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/tmp/z"}),
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "tc-x".into(),
        name: "Read".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: Some(5),
        metadata: None,
    });
    assert_eq!(r.state().agent.tools_in_flight(), 0);

    r.apply(turn_completed_payload());

    assert_eq!(r.state().agent.tools_in_flight(), 0);
}

#[test]
fn error_event_reconciles_stuck_tools() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-err".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 99"}),
    });
    assert_eq!(r.state().agent.tools_in_flight(), 1);

    r.apply(AgentEventPayload::Error {
        message: "LLM stream broken".into(),
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(
        matches!(tc.state, InvocationState::Stale { .. }),
        "got {:?}",
        tc.state.variant_name()
    );
    let obs = &r.state().agent; /* observable now lifted to AgentView methods */
    assert_eq!(
        obs.tools_in_flight(),
        0,
        "Error event must zero tools_in_flight"
    );
    assert!(obs.last_tool().is_none());
}

#[test]
fn awaiting_input_reconciles_stuck_tools() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-ai".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 99"}),
    });
    assert_eq!(r.state().agent.tools_in_flight(), 1);

    r.apply(AgentEventPayload::AwaitingInput);

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Stale { .. }));
    assert_eq!(r.state().agent.tools_in_flight(), 0);
}

#[test]
fn rewound_to_zero_resets_tools_in_flight() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-rw".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "sleep 99"}),
    });
    assert_eq!(r.state().agent.tools_in_flight(), 1);

    r.apply(AgentEventPayload::Rewound { remaining_turns: 0 });

    let obs = &r.state().agent; /* observable now lifted to AgentView methods */
    assert_eq!(
        obs.tools_in_flight(),
        0,
        "Rewound to 0 turns must zero tools_in_flight"
    );
    assert!(obs.last_tool().is_none());
}

#[test]
fn session_resumed_resets_tools_in_flight() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::ToolCall {
        id: "tc-sr".into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "x"}),
    });
    assert_eq!(r.state().agent.tools_in_flight(), 1);

    r.apply(AgentEventPayload::SessionResumed {
        session_id: "new-sess".into(),
        message_count: 0,
    });

    let obs = &r.state().agent; /* observable now lifted to AgentView methods */
    assert_eq!(obs.tools_in_flight(), 0);
    assert!(obs.last_tool().is_none());
}
