use loopal_context::build_system_prompt;
use loopal_tool_api::ToolDefinition;

#[test]
fn explore_subagent_full_prompt() {
    let tools = vec![
        ToolDefinition {
            name: "Read".into(),
            description: "Read a file".into(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "Grep".into(),
            description: "Search file contents".into(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "Bash".into(),
            description: "Execute commands".into(),
            input_schema: serde_json::json!({"type": "object"}),
        },
    ];
    let result = build_system_prompt(
        "Project rules",
        &tools,
        "act",
        "/project",
        "",
        "",
        Some("explore"),
        vec![],
        0,
    );

    // Explore-specific content present
    assert!(
        result.contains("READ-ONLY MODE"),
        "explore fragment should be included"
    );
    // Default sub-agent fragment excluded (explore is specific)
    assert!(
        !result.contains("sub-agent named"),
        "default-subagent should be excluded when explore matches"
    );
    // Core fragments still present
    assert!(
        result.contains("Output Efficiency"),
        "core fragments should be included for sub-agents"
    );
    // Tool usage policy still present
    assert!(
        result.contains("Tool Usage Policy"),
        "tool usage policy should be included for sub-agents"
    );
    // User instructions still present
    assert!(result.contains("Project rules"), "instructions missing");
    // Tool schemas NOT in prompt
    assert!(
        !result.contains("# Available Tools"),
        "tool schemas should not be in system prompt"
    );
}

#[test]
fn root_agent_excludes_agent_fragments() {
    let result = build_system_prompt("Base", &[], "act", "/workspace", "", "", None, vec![], 0);
    // No agent fragments in root prompt
    assert!(
        !result.contains("sub-agent named"),
        "default-subagent should not appear for root agent"
    );
    assert!(
        !result.contains("READ-ONLY MODE"),
        "explore fragment should not appear for root agent"
    );
    // Core fragments present
    assert!(result.contains("Output Efficiency"));
}

#[test]
fn plan_subagent_gets_plan_fragment() {
    let result = build_system_prompt("", &[], "act", "/work", "", "", Some("plan"), vec![], 0);
    assert!(
        result.contains("software architect"),
        "plan fragment should be included"
    );
    assert!(
        !result.contains("sub-agent named"),
        "default-subagent excluded when plan matches"
    );
    assert!(
        !result.contains("READ-ONLY MODE"),
        "explore fragment should not be included for plan agent"
    );
}

#[test]
fn general_subagent_gets_default_fragment() {
    let result = build_system_prompt("", &[], "act", "/work", "", "", Some("general"), vec![], 0);
    // Default sub-agent fragment (fallback for unknown types)
    assert!(
        result.contains("sub-agent named"),
        "default-subagent should be included for unknown agent types"
    );
    // Specialized fragments excluded
    assert!(
        !result.contains("READ-ONLY MODE"),
        "explore fragment should not appear for general"
    );
}
