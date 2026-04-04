use std::collections::HashMap;

use loopal_prompt::{FragmentRegistry, PromptBuilder, PromptContext};
use loopal_prompt_system::system_fragments;
use loopal_tool_api::ToolDefinition;

/// Build a full system prompt using the fragment-based prompt system.
pub fn build_system_prompt(
    instructions: &str,
    tools: &[ToolDefinition],
    mode: &str,
    cwd: &str,
    skills_summary: &str,
    memory: &str,
    agent_type: Option<&str>,
) -> String {
    let mut registry = FragmentRegistry::new(system_fragments());

    // Load user overrides: global (~/.loopal/prompts/) then project (<cwd>/.loopal/prompts/)
    if let Ok(global_dir) = loopal_config::global_prompts_dir() {
        registry.add_overrides_from_path(&global_dir);
    }
    registry.add_overrides_from_path(&std::path::PathBuf::from(cwd).join(".loopal/prompts"));

    let builder = PromptBuilder::new(registry);

    // Tool names/descriptions feed Minijinja conditionals in fragments
    // (e.g. `{% if "Bash" in tool_names %}`). Full JSON schemas are sent
    // separately via ChatParams.tools — no need to duplicate them here.
    let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    let tool_descriptions: HashMap<String, String> = tools
        .iter()
        .map(|t| (t.name.clone(), t.description.clone()))
        .collect();

    let ctx = PromptContext {
        cwd: cwd.to_string(),
        platform: std::env::consts::OS.to_string(),
        date: today(),
        is_git_repo: false, // caller can improve later
        git_branch: None,
        mode: if mode.is_empty() {
            "act".to_string()
        } else {
            mode.to_string()
        },
        tool_names,
        tool_descriptions,
        instructions: instructions.to_string(),
        memory: memory.to_string(),
        skills_summary: skills_summary.to_string(),
        features: Vec::new(),
        agent_name: None,
        agent_type: agent_type.map(String::from),
    };

    builder.build(&ctx)
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
