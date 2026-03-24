use loopal_context::build_system_prompt;
use loopal_tool_api::ToolDefinition;

#[test]
fn includes_instructions() {
    let result = build_system_prompt("You are helpful.", &[], "act", "/tmp", "", "");
    assert!(result.contains("You are helpful."));
}

#[test]
fn includes_tool_schemas() {
    let tools = vec![ToolDefinition {
        name: "read".into(),
        description: "Read a file".into(),
        input_schema: serde_json::json!({"type": "object"}),
    }];
    let result = build_system_prompt("Base", &tools, "act", "/workspace", "", "");
    assert!(result.contains("# Available Tools"));
    assert!(result.contains("## read"));
    assert!(result.contains("Read a file"));
}

#[test]
fn includes_fragments() {
    let result = build_system_prompt("Base", &[], "act", "/workspace", "", "");
    // Core fragments should be present
    assert!(
        result.contains("Output Efficiency"),
        "output efficiency fragment missing"
    );
    assert!(
        result.contains("Executing Actions with Care"),
        "safety fragment missing"
    );
}

#[test]
fn includes_environment() {
    let result = build_system_prompt("Base", &[], "act", "/Users/dev/project", "", "");
    assert!(result.contains("/Users/dev/project"), "cwd not rendered");
}

#[test]
fn includes_skills() {
    let skills = "# Available Skills\n- /commit: Generate a git commit message";
    let result = build_system_prompt("Base", &[], "act", "/workspace", skills, "");
    assert!(result.contains("Available Skills"));
    assert!(result.contains("/commit"));
}

#[test]
fn includes_memory() {
    let result = build_system_prompt(
        "Base",
        &[],
        "act",
        "/workspace",
        "",
        "## Key Patterns\n- Use DI",
    );
    assert!(result.contains("# Project Memory"));
    assert!(result.contains("Key Patterns"));
}

#[test]
fn empty_memory_no_section() {
    let result = build_system_prompt("Base", &[], "act", "/workspace", "", "");
    assert!(!result.contains("Project Memory"));
}

#[test]
fn tool_conditional_fragments() {
    // With Bash tool → bash guidelines should appear
    let tools = vec![ToolDefinition {
        name: "Bash".into(),
        description: "Execute commands".into(),
        input_schema: serde_json::json!({"type": "object"}),
    }];
    let result = build_system_prompt("Base", &tools, "act", "/workspace", "", "");
    assert!(
        result.contains("Bash Tool Guidelines"),
        "bash guidelines missing when Bash tool present"
    );

    // Without Bash tool → no bash guidelines
    let result_no_bash = build_system_prompt("Base", &[], "act", "/workspace", "", "");
    assert!(
        !result_no_bash.contains("Bash Tool Guidelines"),
        "bash guidelines should not appear without Bash"
    );
}

#[test]
fn report_token_usage() {
    use loopal_context::estimate_tokens;

    let tools: Vec<ToolDefinition> = [
        (
            "Read",
            "Read a file from the filesystem. Returns content with line numbers.",
        ),
        (
            "Write",
            "Write content to a file. Creates parent directories if needed.",
        ),
        ("Edit", "Perform exact string replacement in a file."),
        ("MultiEdit", "Apply multiple sequential edits atomically."),
        (
            "Bash",
            "Execute a bash command. Captures stdout and stderr.",
        ),
        ("Glob", "Find files matching a glob pattern."),
        ("Grep", "Search file contents using a regex pattern."),
        ("Ls", "List directory contents."),
        ("Fetch", "Download a URL."),
        ("WebSearch", "Search the web using Tavily API."),
        ("AskUser", "Present questions to the user."),
        ("EnterPlanMode", "Switch into plan mode."),
        ("ExitPlanMode", "Exit plan mode."),
        ("Agent", "Spawn a sub-agent."),
        ("TaskCreate", "Create a new task."),
        ("TaskUpdate", "Update an existing task."),
        ("TaskList", "List all tasks."),
        ("TaskGet", "Get task details by ID."),
        ("SendMessage", "Send a message to another agent."),
        ("EnterWorktree", "Create a git worktree."),
        ("ExitWorktree", "Exit the current worktree."),
    ]
    .iter()
    .map(|(n, d)| ToolDefinition {
        name: n.to_string(),
        description: d.to_string(),
        input_schema: serde_json::json!({"type":"object","properties":{}}),
    })
    .collect();

    let instr = "You are a helpful coding assistant.\n\nAlways respond in Chinese.";
    let mem = "## Architecture\n- 17 Rust crates\n- 200-line limit";
    let skills = "# Available Skills\n- /commit: Git commit\n- /review-pr: Review PR";

    let bare = build_system_prompt("", &[], "act", "/project", "", "");
    let with_tools = build_system_prompt("", &tools, "act", "/project", "", "");
    let full_act = build_system_prompt(instr, &tools, "act", "/project", skills, mem);
    let full_plan = build_system_prompt(instr, &tools, "plan", "/project", skills, mem);

    let t_bare = estimate_tokens(&bare);
    let t_tools = estimate_tokens(&with_tools);
    let t_act = estimate_tokens(&full_act);
    let t_plan = estimate_tokens(&full_plan);

    eprintln!("\n=== System Prompt Token Analysis ===");
    eprintln!(
        "Fragments only:              {} tokens ({} chars)",
        t_bare,
        bare.len()
    );
    eprintln!("Fragments + 21 tool schemas: {t_tools} tokens");
    eprintln!(
        "Full (act, 21 tools):        {} tokens ({} chars)",
        t_act,
        full_act.len()
    );
    eprintln!(
        "Full (plan, 21 tools):       {} tokens ({} chars)",
        t_plan,
        full_plan.len()
    );
    eprintln!("Plan overhead:               +{} tokens", t_plan - t_act);
    eprintln!("--- Breakdown ---");
    eprintln!("  Behavior fragments: {t_bare} tokens");
    eprintln!("  Tool schemas:       {} tokens", t_tools - t_bare);
    eprintln!("  Instructions:       {} tokens", estimate_tokens(instr));
    eprintln!("  Skills:             {} tokens", estimate_tokens(skills));
    eprintln!(
        "  Memory:             {} tokens",
        estimate_tokens(&format!("# Project Memory\n{mem}"))
    );
}
