use loopagent_types::permission::{PermissionDecision, PermissionMode};
use loopagent_types::tool::Tool;

/// Check whether a tool is allowed under the given permission mode.
pub fn check_permission(mode: &PermissionMode, tool: &dyn Tool) -> PermissionDecision {
    mode.check(tool.permission())
}
