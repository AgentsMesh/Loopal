use std::time::Instant;

use loopal_tool_invocation::{InvocationId, Outcome, ToolInvocation, TransitionCmd, transition};
use serde_json::Value;
use tracing::warn;

use super::agent_conversation::AgentConversation;
use super::truncate::truncate_json;
use super::types::SessionMessage;

pub(crate) fn handle_server_tool_use(
    conv: &mut AgentConversation,
    id: String,
    name: String,
    input: Value,
) {
    let Ok(invocation_id) = InvocationId::new(id) else {
        warn!("ignoring server_tool_use with empty id");
        return;
    };
    conv.flush_streaming();
    let summary = format!("{}({})", name, truncate_json(&input, 60));
    let invocation =
        ToolInvocation::start(invocation_id, name, summary, Some(input), Instant::now());
    if let Some(last) = conv.messages.last_mut()
        && last.role == "assistant"
    {
        last.tool_calls.push(invocation);
        return;
    }
    conv.messages.push(SessionMessage {
        role: "assistant".to_string(),
        tool_calls: vec![invocation],
        ..Default::default()
    });
}

pub(crate) fn handle_server_tool_result(
    conv: &mut AgentConversation,
    tool_use_id: &str,
    content: &Value,
) {
    let Ok(target_id) = InvocationId::new(tool_use_id.to_string()) else {
        return;
    };
    let Some(msg) = conv.messages.last_mut() else {
        return;
    };
    if let Some(tc) = msg.tool_calls.iter_mut().rfind(|tc| tc.id == target_id) {
        let formatted = format_server_tool_content(content);
        let outcome = Outcome::Success { content: formatted };
        let prev = tc.clone();
        if let Ok(next) = transition(prev, TransitionCmd::Complete(outcome), Instant::now()) {
            tc.state = next.state;
        }
    }
}

pub fn format_server_tool_content(content: &Value) -> String {
    if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(arr) = content.as_array() {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|v| {
                v.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    content.to_string()
}
