use loopal_protocol::AgentEventPayload;
use loopal_view_state::ViewStateReducer;

fn tool_call(id: &str, name: &str) -> AgentEventPayload {
    AgentEventPayload::ToolCall {
        id: id.into(),
        name: name.into(),
        input: serde_json::json!({}),
    }
}

#[test]
fn batch_start_assigns_batch_id_to_matching_pending() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(tool_call("b", "Bash"));
    r.apply(AgentEventPayload::ToolBatchStart {
        tool_ids: vec!["a".into(), "b".into()],
    });

    let tcs = &r.state().agent.conversation.messages[0].tool_calls;
    assert!(tcs[0].batch_id.is_some());
    assert!(tcs[1].batch_id.is_some());
    assert_eq!(tcs[0].batch_id, tcs[1].batch_id);
}

#[test]
fn batch_start_skips_unmatched_ids() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(tool_call("b", "Bash"));
    r.apply(AgentEventPayload::ToolBatchStart {
        tool_ids: vec!["a".into(), "x".into()],
    });

    let tcs = &r.state().agent.conversation.messages[0].tool_calls;
    assert!(tcs[0].batch_id.is_some());
    assert!(tcs[1].batch_id.is_none());
}

#[test]
fn batch_start_ignores_terminal_tools() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    r.apply(tool_call("b", "Bash"));
    r.apply(AgentEventPayload::ToolBatchStart {
        tool_ids: vec!["a".into(), "b".into()],
    });

    let tcs = &r.state().agent.conversation.messages[0].tool_calls;
    assert!(
        tcs[0].batch_id.is_none(),
        "done invocation should not be batched"
    );
    assert!(
        tcs[1].batch_id.is_some(),
        "active invocation should be batched"
    );
}

#[test]
fn batch_start_with_empty_ids_is_noop() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolBatchStart { tool_ids: vec![] });

    let tcs = &r.state().agent.conversation.messages[0].tool_calls;
    assert!(tcs[0].batch_id.is_none());
}
