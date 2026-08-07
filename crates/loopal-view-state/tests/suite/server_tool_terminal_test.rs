use loopal_protocol::AgentEventPayload;
use loopal_tool_invocation::{InvocationState, StaleReason};
use loopal_view_state::ViewStateReducer;

fn server_tool_use(id: &str) -> AgentEventPayload {
    AgentEventPayload::ServerToolUse {
        id: id.into(),
        name: "web_search".into(),
        input: serde_json::json!({"query": "rust"}),
    }
}

#[test]
fn discarded_server_tool_terminalizes_an_earlier_row() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(server_tool_use("search-1"));
    reducer.apply(AgentEventPayload::ProviderWarning {
        message: "provider stream interrupted".into(),
    });
    reducer.apply(AgentEventPayload::ServerToolDiscarded {
        tool_use_id: "search-1".into(),
        reason: StaleReason::IncompleteModelResponse,
    });

    let invocation = reducer
        .state()
        .agent
        .conversation
        .messages
        .iter()
        .flat_map(|message| &message.tool_calls)
        .find(|tool| tool.id.as_str() == "search-1")
        .expect("server tool invocation");
    assert!(matches!(
        invocation.state,
        InvocationState::Stale {
            reason: StaleReason::IncompleteModelResponse,
            ..
        }
    ));
}

#[test]
fn server_tool_result_can_complete_an_earlier_row() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(server_tool_use("search-2"));
    reducer.apply(AgentEventPayload::ProviderWarning {
        message: "delayed result".into(),
    });
    reducer.apply(AgentEventPayload::ServerToolResult {
        tool_use_id: "search-2".into(),
        content: serde_json::json!({"text": "found"}),
    });

    let invocation = reducer
        .state()
        .agent
        .conversation
        .messages
        .iter()
        .flat_map(|message| &message.tool_calls)
        .find(|tool| tool.id.as_str() == "search-2")
        .expect("server tool invocation");
    assert!(matches!(invocation.state, InvocationState::Done { .. }));
}
