//! Hook input construction — builds the JSON payload for each event.
//!
//! Information Expert: this module knows what data each event needs,
//! so the payload construction logic lives here.

use loopal_config::HookEvent;
use serde_json::json;

/// Contextual data available to hooks. Not all fields are populated for
/// every event — `build_hook_input` selects what's relevant.
#[derive(Debug, Default)]
pub struct HookContext<'a> {
    // Tool events
    pub tool_name: Option<&'a str>,
    pub tool_input: Option<&'a serde_json::Value>,
    pub tool_output: Option<&'a str>,
    pub is_error: Option<bool>,
    // Session events
    pub session_id: Option<&'a str>,
    pub cwd: Option<&'a str>,
    // Stop event
    pub stop_reason: Option<&'a str>,
}

/// Build the JSON payload sent to a hook's stdin based on the event type.
pub fn build_hook_input(event: HookEvent, ctx: &HookContext<'_>) -> serde_json::Value {
    match event {
        HookEvent::PreToolUse => json!({
            "event": "pre_tool_use",
            "tool_name": ctx.tool_name,
            "tool_input": ctx.tool_input,
        }),
        HookEvent::PostToolUse => json!({
            "event": "post_tool_use",
            "tool_name": ctx.tool_name,
            "tool_input": ctx.tool_input,
            "tool_output": ctx.tool_output,
            "is_error": ctx.is_error,
        }),
        HookEvent::PreRequest => json!({
            "event": "pre_request",
            "session_id": ctx.session_id,
        }),
        HookEvent::PostInput => json!({
            "event": "post_input",
            "session_id": ctx.session_id,
        }),
        HookEvent::SessionStart => json!({
            "event": "session_start",
            "session_id": ctx.session_id,
            "cwd": ctx.cwd,
        }),
        HookEvent::SessionEnd => json!({
            "event": "session_end",
            "session_id": ctx.session_id,
        }),
        HookEvent::Stop => json!({
            "event": "stop",
            "reason": ctx.stop_reason,
        }),
        HookEvent::PreCompact => json!({
            "event": "pre_compact",
            "session_id": ctx.session_id,
        }),
    }
}
