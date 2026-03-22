use loopal_tool_api::ToolDefinition;

/// Build a full system prompt from parts.
///
/// `skills_summary` is a pre-formatted section listing available skills.
/// Pass an empty string when no skills are loaded.
pub fn build_system_prompt(
    instructions: &str,
    tools: &[ToolDefinition],
    mode_suffix: &str,
    cwd: &str,
    skills_summary: &str,
    memory: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(instructions.to_string());

    // Inject working directory so the LLM knows where it is
    parts.push(format!(
        "\n\n# Working Directory\nYour current working directory is: {}\nAll relative file paths are resolved from this directory. Use relative paths when possible.",
        cwd
    ));

    // Inject project memory if present
    if !memory.is_empty() {
        parts.push(format!("\n\n# Project Memory\n{memory}"));
    }

    if !tools.is_empty() {
        let mut tool_section = String::from("\n\n# Available Tools\n");
        for tool in tools {
            tool_section.push_str(&format!(
                "\n## {}\n{}\nParameters: {}\n",
                tool.name, tool.description, tool.input_schema
            ));
        }
        parts.push(tool_section);
    }

    if !skills_summary.is_empty() {
        parts.push(format!("\n\n{skills_summary}"));
    }

    if !mode_suffix.is_empty() {
        parts.push(format!("\n\n{mode_suffix}"));
    }

    parts.join("")
}
