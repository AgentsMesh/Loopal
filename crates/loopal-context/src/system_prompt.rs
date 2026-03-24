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
) -> String {
    let mut registry = FragmentRegistry::new(system_fragments());

    // Load user overrides: global (~/.loopal/prompts/) then project (<cwd>/.loopal/prompts/)
    if let Ok(global_dir) = loopal_config::global_prompts_dir() {
        registry.add_overrides_from_path(&global_dir);
    }
    registry.add_overrides_from_path(&std::path::PathBuf::from(cwd).join(".loopal/prompts"));

    let builder = PromptBuilder::new(registry);

    let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    let tool_descriptions: HashMap<String, String> = tools
        .iter()
        .map(|t| (t.name.clone(), t.description.clone()))
        .collect();

    // Build tool schema section (kept as-is for LLM function calling)
    let tools_section = if tools.is_empty() {
        String::new()
    } else {
        let mut s = String::from("# Available Tools\n");
        for tool in tools {
            s.push_str(&format!(
                "\n## {}\n{}\nParameters: {}\n",
                tool.name, tool.description, tool.input_schema
            ));
        }
        s
    };

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
        agent_type: None,
    };

    let mut prompt = builder.build(&ctx);

    // Append tool schemas after fragments (LLM needs the JSON schemas)
    if !tools_section.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&tools_section);
    }

    prompt
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
