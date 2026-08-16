use super::build_kernel_depth_test::{build_or_panic, empty_config};

#[tokio::test]
async fn workflow_tools_are_root_only_and_execution_gated() {
    let disabled = build_or_panic(&empty_config(), false, 0, None).await;
    assert!(disabled.get_tool("workflow_start").is_none());

    let mut enabled_config = empty_config();
    enabled_config.settings.workflow.execution_enabled = true;
    enabled_config.settings.workflow.policy = loopal_config::OrchestrationPolicy::Explicit;
    let root = build_or_panic(&enabled_config, false, 0, None).await;
    for name in [
        "workflow_start",
        "workflow_get",
        "workflow_wait",
        "workflow_cancel",
    ] {
        assert!(root.get_tool(name).is_some(), "root is missing {name}");
    }

    let child = build_or_panic(&enabled_config, false, 1, None).await;
    assert!(child.get_tool("workflow_start").is_none());
}
