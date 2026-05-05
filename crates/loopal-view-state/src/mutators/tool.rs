use loopal_protocol::AgentStatus;

use crate::conversation::{server_tool_display, tool_result_handler};
use crate::state::SessionViewState;

pub(super) fn tool_call(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> bool {
    let obs = &mut state.agent.observable;
    obs.tool_count = obs.tool_count.saturating_add(1);
    obs.tools_in_flight = obs.tools_in_flight.saturating_add(1);
    obs.last_tool = Some(extract_key_param(name, input));
    obs.status = AgentStatus::Running;
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    tool_result_handler::handle_tool_call(conv, id.to_string(), name.to_string(), input.clone());
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tool_result(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    result: &str,
    is_error: bool,
    duration_ms: Option<u64>,
    metadata: Option<serde_json::Value>,
) -> bool {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    tool_result_handler::handle_tool_result(
        conv,
        tool_result_handler::ToolResultParams {
            id: id.to_string(),
            name: name.to_string(),
            result: result.to_string(),
            is_error,
            duration_ms,
            metadata,
        },
    );
    let obs = &mut state.agent.observable;
    obs.tools_in_flight = obs.tools_in_flight.saturating_sub(1);
    obs.status = AgentStatus::Running;
    true
}

pub(super) fn tool_batch_start(state: &mut SessionViewState, tool_ids: &[String]) -> bool {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    tool_result_handler::handle_tool_batch_start(conv, tool_ids.to_vec());
    true
}

pub(super) fn tool_progress(state: &mut SessionViewState, id: &str, output_tail: &str) -> bool {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    tool_result_handler::handle_tool_progress(conv, id.to_string(), output_tail.to_string());
    true
}

pub(super) fn server_tool_use(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> bool {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    server_tool_display::handle_server_tool_use(conv, id.to_string(), name.to_string(), input);
    state.agent.observable.status = AgentStatus::Running;
    true
}

pub(super) fn server_tool_result(
    state: &mut SessionViewState,
    tool_use_id: &str,
    content: &serde_json::Value,
) -> bool {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    server_tool_display::handle_server_tool_result(conv, tool_use_id, content);
    state.agent.observable.status = AgentStatus::Running;
    true
}

fn extract_key_param(tool_name: &str, input: &serde_json::Value) -> String {
    let key = match tool_name {
        "Read" | "Write" | "Edit" | "MultiEdit" => "file_path",
        "Bash" => "command",
        "Grep" | "Glob" => "pattern",
        _ => return tool_name.to_string(),
    };
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| {
            if s.len() > 40 {
                let t: String = s.chars().take(37).collect();
                format!("{tool_name}({t}...)")
            } else {
                format!("{tool_name}({s})")
            }
        })
        .unwrap_or_else(|| tool_name.to_string())
}
