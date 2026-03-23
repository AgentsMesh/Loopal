use loopal_context::build_system_prompt;
use loopal_tool_api::ToolDefinition;

#[test]
fn test_build_basic() {
    let result = build_system_prompt("You are helpful.", &[], "", "/tmp", "", "");
    assert!(result.contains("You are helpful."));
    assert!(result.contains("/tmp"));
}

#[test]
fn test_build_with_tools() {
    let tools = vec![ToolDefinition {
        name: "read".into(),
        description: "Read a file".into(),
        input_schema: serde_json::json!({"type": "object"}),
    }];
    let result = build_system_prompt("Base", &tools, "", "/workspace", "", "");
    assert!(result.contains("# Available Tools"));
    assert!(result.contains("## read"));
    assert!(result.contains("Read a file"));
}

#[test]
fn test_build_with_mode_suffix() {
    let result = build_system_prompt("Base", &[], "PLAN MODE", "/workspace", "", "");
    assert!(result.contains("PLAN MODE"));
}

#[test]
fn test_build_empty_tools_no_section() {
    let result = build_system_prompt("Base", &[], "", "/workspace", "", "");
    assert!(!result.contains("Available Tools"));
}

#[test]
fn test_build_includes_working_directory() {
    let result = build_system_prompt("Base", &[], "", "/Users/dev/project", "", "");
    assert!(result.contains("Working Directory"));
    assert!(result.contains("/Users/dev/project"));
    assert!(result.contains("relative"));
}

#[test]
fn test_build_with_skills_summary() {
    let skills = "# Available Skills\n- /commit: Generate a git commit message";
    let result = build_system_prompt("Base", &[], "", "/workspace", skills, "");
    assert!(result.contains("Available Skills"));
    assert!(result.contains("/commit"));
}

#[test]
fn test_build_empty_skills_no_section() {
    let result = build_system_prompt("Base", &[], "", "/workspace", "", "");
    assert!(!result.contains("Available Skills"));
}

#[test]
fn test_build_with_memory() {
    let result = build_system_prompt(
        "Base",
        &[],
        "",
        "/workspace",
        "",
        "## Key Patterns\n- Use DI",
    );
    assert!(result.contains("# Project Memory"));
    assert!(result.contains("Key Patterns"));
}

#[test]
fn test_build_empty_memory_no_section() {
    let result = build_system_prompt("Base", &[], "", "/workspace", "", "");
    assert!(!result.contains("Project Memory"));
}
