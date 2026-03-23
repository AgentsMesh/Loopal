use loopal_runtime::check_permission;
use loopal_tool_api::{PermissionDecision, PermissionLevel, PermissionMode};
use loopal_tool_api::{Tool, ToolContext, ToolResult};

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
    ) -> Result<ToolResult, loopal_error::LoopalError> {
        Ok(ToolResult::success("ok"))
    }
}

// =====================================================
// Bypass mode tests
// =====================================================

#[test]
fn test_bypass_readonly_allows() {
    let tool = DummyTool {
        perm: PermissionLevel::ReadOnly,
    };
    assert_eq!(
        check_permission(&PermissionMode::Bypass, &tool),
        PermissionDecision::Allow
    );
}

#[test]
fn test_bypass_supervised_allows() {
    let tool = DummyTool {
        perm: PermissionLevel::Supervised,
    };
    assert_eq!(
        check_permission(&PermissionMode::Bypass, &tool),
        PermissionDecision::Allow
    );
}

#[test]
fn test_bypass_dangerous_allows() {
    let tool = DummyTool {
        perm: PermissionLevel::Dangerous,
    };
    assert_eq!(
        check_permission(&PermissionMode::Bypass, &tool),
        PermissionDecision::Allow
    );
}

// =====================================================
// Supervised mode tests
// =====================================================

#[test]
fn test_supervised_readonly_allows() {
    let tool = DummyTool {
        perm: PermissionLevel::ReadOnly,
    };
    assert_eq!(
        check_permission(&PermissionMode::Supervised, &tool),
        PermissionDecision::Allow
    );
}

#[test]
fn test_supervised_supervised_asks() {
    let tool = DummyTool {
        perm: PermissionLevel::Supervised,
    };
    assert_eq!(
        check_permission(&PermissionMode::Supervised, &tool),
        PermissionDecision::Ask
    );
}

#[test]
fn test_supervised_dangerous_asks() {
    let tool = DummyTool {
        perm: PermissionLevel::Dangerous,
    };
    assert_eq!(
        check_permission(&PermissionMode::Supervised, &tool),
        PermissionDecision::Ask
    );
}
