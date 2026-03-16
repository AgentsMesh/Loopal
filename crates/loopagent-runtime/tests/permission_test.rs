use loopagent_runtime::check_permission;
use loopagent_types::permission::{PermissionDecision, PermissionLevel, PermissionMode};
use loopagent_types::tool::{Tool, ToolContext, ToolResult};

/// A dummy tool that returns a configurable permission level.
struct DummyTool {
    perm: PermissionLevel,
}

#[async_trait::async_trait]
impl Tool for DummyTool {
    fn name(&self) -> &str {
        "DummyTool"
    }
    fn description(&self) -> &str {
        "a dummy tool for testing"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission(&self) -> PermissionLevel {
        self.perm
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, loopagent_types::error::LoopAgentError> {
        Ok(ToolResult::success("ok"))
    }
}

// =====================================================
// Default mode tests
// =====================================================

#[test]
fn test_default_mode_readonly_allows() {
    let tool = DummyTool { perm: PermissionLevel::ReadOnly };
    assert_eq!(check_permission(&PermissionMode::Default, &tool), PermissionDecision::Allow);
}

#[test]
fn test_default_mode_supervised_asks() {
    let tool = DummyTool { perm: PermissionLevel::Supervised };
    assert_eq!(check_permission(&PermissionMode::Default, &tool), PermissionDecision::Ask);
}

#[test]
fn test_default_mode_dangerous_asks() {
    let tool = DummyTool { perm: PermissionLevel::Dangerous };
    assert_eq!(check_permission(&PermissionMode::Default, &tool), PermissionDecision::Ask);
}

// =====================================================
// AcceptEdits mode tests
// =====================================================

#[test]
fn test_accept_edits_readonly_allows() {
    let tool = DummyTool { perm: PermissionLevel::ReadOnly };
    assert_eq!(check_permission(&PermissionMode::AcceptEdits, &tool), PermissionDecision::Allow);
}

#[test]
fn test_accept_edits_supervised_allows() {
    let tool = DummyTool { perm: PermissionLevel::Supervised };
    assert_eq!(check_permission(&PermissionMode::AcceptEdits, &tool), PermissionDecision::Allow);
}

#[test]
fn test_accept_edits_dangerous_asks() {
    let tool = DummyTool { perm: PermissionLevel::Dangerous };
    assert_eq!(check_permission(&PermissionMode::AcceptEdits, &tool), PermissionDecision::Ask);
}

// =====================================================
// BypassPermissions mode tests
// =====================================================

#[test]
fn test_bypass_readonly_allows() {
    let tool = DummyTool { perm: PermissionLevel::ReadOnly };
    assert_eq!(check_permission(&PermissionMode::BypassPermissions, &tool), PermissionDecision::Allow);
}

#[test]
fn test_bypass_supervised_allows() {
    let tool = DummyTool { perm: PermissionLevel::Supervised };
    assert_eq!(check_permission(&PermissionMode::BypassPermissions, &tool), PermissionDecision::Allow);
}

#[test]
fn test_bypass_dangerous_allows() {
    let tool = DummyTool { perm: PermissionLevel::Dangerous };
    assert_eq!(check_permission(&PermissionMode::BypassPermissions, &tool), PermissionDecision::Allow);
}

// =====================================================
// Plan mode tests
// =====================================================

#[test]
fn test_plan_readonly_allows() {
    let tool = DummyTool { perm: PermissionLevel::ReadOnly };
    assert_eq!(check_permission(&PermissionMode::Plan, &tool), PermissionDecision::Allow);
}

#[test]
fn test_plan_supervised_denies() {
    let tool = DummyTool { perm: PermissionLevel::Supervised };
    assert_eq!(check_permission(&PermissionMode::Plan, &tool), PermissionDecision::Deny);
}

#[test]
fn test_plan_dangerous_denies() {
    let tool = DummyTool { perm: PermissionLevel::Dangerous };
    assert_eq!(check_permission(&PermissionMode::Plan, &tool), PermissionDecision::Deny);
}
