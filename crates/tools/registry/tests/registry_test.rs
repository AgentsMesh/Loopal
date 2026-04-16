use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolDispatch, ToolResult};
use loopal_tools::ToolRegistry;
use serde_json::{Value, json};

struct MockTool {
    tool_name: String,
    tool_description: String,
}

impl MockTool {
    fn new(name: &str) -> Self {
        Self {
            tool_name: name.to_string(),
            tool_description: format!("Mock tool: {name}"),
        }
    }
}

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success("mock result"))
    }
}

#[test]
fn test_register_and_get() {
    let registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("TestTool")));

    let tool = registry.get("TestTool");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name(), "TestTool");
}

#[test]
fn test_get_unknown_returns_none() {
    let registry = ToolRegistry::new();
    assert!(registry.get("NonExistent").is_none());
}

#[test]
fn test_list_returns_all_tools() {
    let registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("ToolA")));
    registry.register(Box::new(MockTool::new("ToolB")));
    registry.register(Box::new(MockTool::new("ToolC")));

    let tools = registry.list();
    assert_eq!(tools.len(), 3);

    let mut names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    names.sort();
    assert_eq!(names, vec!["ToolA", "ToolB", "ToolC"]);
}

#[test]
fn test_to_definitions() {
    let registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("Beta")));
    registry.register(Box::new(MockTool::new("Alpha")));

    let defs = registry.to_definitions();
    assert_eq!(defs.len(), 2);
    // to_definitions sorts by name
    assert_eq!(defs[0].name, "Alpha");
    assert_eq!(defs[1].name, "Beta");
    assert_eq!(defs[0].description, "Mock tool: Alpha");
    assert_eq!(defs[1].description, "Mock tool: Beta");
}

#[test]
fn test_register_multiple_tools() {
    let registry = ToolRegistry::new();
    for i in 0..5 {
        registry.register(Box::new(MockTool::new(&format!("Tool{i}"))));
    }
    assert_eq!(registry.list().len(), 5);
}

#[test]
fn test_register_overwrites_same_name() {
    let registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("Dup")));
    registry.register(Box::new(MockTool::new("Dup")));

    // Should still have only one tool with that name
    assert_eq!(registry.list().len(), 1);
    assert!(registry.get("Dup").is_some());
}

#[test]
fn test_default_creates_empty_registry() {
    let registry = ToolRegistry::default();
    assert!(registry.list().is_empty());
}

#[test]
fn test_unregister_removes_tool() {
    let registry = ToolRegistry::new();
    registry.register(Box::new(MockTool::new("Keep")));
    registry.register(Box::new(MockTool::new("Remove")));

    assert!(registry.unregister("Remove"));
    assert!(registry.get("Remove").is_none());
    assert!(registry.get("Keep").is_some());
    assert_eq!(registry.list().len(), 1);
}

#[test]
fn test_unregister_nonexistent_returns_false() {
    let registry = ToolRegistry::new();
    assert!(!registry.unregister("Ghost"));
}

/// Contract: runner-direct tools (AskUser, EnterPlanMode, ExitPlanMode) declare
/// `ToolDispatch::RunnerDirect`; all other builtin tools default to `Pipeline`.
#[test]
fn dispatch_contract_for_builtin_tools() {
    use loopal_tool_ask_user::AskUserTool;
    use loopal_tool_plan_mode::{EnterPlanModeTool, ExitPlanModeTool};
    use loopal_tool_read::ReadTool;

    // Runner-direct tools
    assert_eq!(AskUserTool.dispatch(), ToolDispatch::RunnerDirect);
    assert_eq!(EnterPlanModeTool.dispatch(), ToolDispatch::RunnerDirect);
    assert_eq!(ExitPlanModeTool.dispatch(), ToolDispatch::RunnerDirect);

    // Pipeline tools (default)
    assert_eq!(ReadTool.dispatch(), ToolDispatch::Pipeline);
    assert_eq!(MockTool::new("Foo").dispatch(), ToolDispatch::Pipeline);
}
