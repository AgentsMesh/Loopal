use loopal_tool_api::Tool;
use loopal_tool_api::{PermissionDecision, PermissionMode};

/// Check whether a tool is allowed under the given permission mode.
pub fn check_permission(mode: &PermissionMode, tool: &dyn Tool) -> PermissionDecision {
    mode.check(tool.permission())
}
