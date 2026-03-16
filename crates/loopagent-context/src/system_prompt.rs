use loopagent_types::tool::ToolDefinition;

/// Build a full system prompt from parts.
pub fn build_system_prompt(
    instructions: &str,
    tools: &[ToolDefinition],
    mode_suffix: &str,
    cwd: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(instructions.to_string());

    // Inject working directory so the LLM knows where it is
    parts.push(format!(
        "\n\n# Working Directory\nYour current working directory is: {}\nAll relative file paths are resolved from this directory. Use relative paths when possible.",
        cwd
    ));

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

    if !mode_suffix.is_empty() {
        parts.push(format!("\n\n{mode_suffix}"));
    }

    parts.join("")
}
