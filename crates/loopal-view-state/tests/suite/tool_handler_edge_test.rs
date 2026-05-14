use loopal_protocol::AgentEventPayload;
use loopal_tool_invocation::InvocationState;
use loopal_view_state::ViewStateReducer;

fn tool_call(id: &str, name: &str) -> AgentEventPayload {
    AgentEventPayload::ToolCall {
        id: id.into(),
        name: name.into(),
        input: serde_json::json!({}),
    }
}

#[test]
fn tool_result_for_unknown_id_does_not_panic() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "nonexistent".into(),
        name: "Bash".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Pending));
}

#[test]
fn tool_result_with_empty_id_is_rejected() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "".into(),
        name: "Bash".into(),
        result: "should not apply".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Pending));
}

#[test]
fn tool_call_with_empty_id_is_rejected() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("", "Bash"));
    assert!(r.state().agent.conversation.messages.is_empty());
}

#[test]
fn tool_call_with_empty_id_does_not_leak_tools_in_flight() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("", "Bash"));
    let obs = &r.state().agent; /* observable now lifted to AgentView methods */
    assert_eq!(
        obs.tools_in_flight(),
        0,
        "empty-id must not increment in-flight"
    );
    assert_eq!(obs.tool_count(), 0, "empty-id must not increment count");
    assert!(obs.last_tool().is_none(), "empty-id must not set last_tool");
}

#[test]
fn duplicate_tool_result_rejected_after_done() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "first".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "second".into(),
        is_error: true,
        duration_ms: None,
        metadata: None,
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    let outcome = tc.state.outcome().expect("Done");
    assert_eq!(outcome.content(), "first");
    assert!(!outcome.is_error());
}

#[test]
fn tool_progress_after_done_is_rejected() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "done".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    r.apply(AgentEventPayload::ToolProgress {
        id: "a".into(),
        name: "Bash".into(),
        output_tail: "late progress".into(),
        elapsed_ms: 999,
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Done { .. }));
}

#[test]
fn empty_id_tool_call_does_not_flush_streaming() {
    let mut r = ViewStateReducer::new("main");
    r.apply(AgentEventPayload::Stream {
        text: "partial ".into(),
    });
    r.apply(tool_call("", "Bash"));

    let streaming = &r.state().agent.conversation.streaming_text;
    assert_eq!(streaming, "partial ");
}

#[test]
fn rejected_tool_result_does_not_decrement_tools_in_flight() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    assert_eq!(r.state().agent.tools_in_flight(), 1);

    r.apply(AgentEventPayload::ToolResult {
        id: "nonexistent".into(),
        name: "Bash".into(),
        result: "stray".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    assert_eq!(
        r.state().agent.tools_in_flight(),
        1,
        "unknown-id tool_result must not decrement in-flight"
    );

    r.apply(AgentEventPayload::ToolResult {
        id: "".into(),
        name: "Bash".into(),
        result: "empty".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    assert_eq!(
        r.state().agent.tools_in_flight(),
        1,
        "empty-id tool_result must not decrement in-flight"
    );
}

#[test]
fn duplicate_tool_result_does_not_double_decrement() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(tool_call("b", "Bash"));
    assert_eq!(r.state().agent.tools_in_flight(), 2);

    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "first".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    assert_eq!(r.state().agent.tools_in_flight(), 1);

    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "duplicate".into(),
        is_error: true,
        duration_ms: None,
        metadata: None,
    });
    assert_eq!(
        r.state().agent.tools_in_flight(),
        1,
        "duplicate tool_result must not decrement again"
    );
}

#[test]
fn duplicate_tool_call_is_rejected() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("dup", "Bash"));
    assert_eq!(r.state().agent.tools_in_flight(), 1);
    assert_eq!(r.state().agent.tool_count(), 1);

    r.apply(tool_call("dup", "Bash"));
    assert_eq!(
        r.state().agent.tools_in_flight(),
        1,
        "duplicate tool_call must not increment in-flight"
    );
    assert_eq!(
        r.state().agent.tool_count(),
        1,
        "duplicate tool_call must not increment count"
    );
    let total_tool_calls: usize = r
        .state()
        .agent
        .conversation
        .messages
        .iter()
        .map(|m| m.tool_calls.len())
        .sum();
    assert_eq!(
        total_tool_calls, 1,
        "duplicate tool_call must not append second invocation"
    );
}
