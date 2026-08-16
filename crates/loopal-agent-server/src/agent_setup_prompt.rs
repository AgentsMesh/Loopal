pub(crate) struct SessionPromptOptions<'a> {
    pub(crate) mode: &'a str,
    pub(crate) agent_type: Option<&'a str>,
    pub(crate) depth: u32,
    pub(crate) tool_defs: &'a [loopal_tool_api::ToolDefinition],
    pub(crate) features: Vec<String>,
}

pub(crate) async fn build_session_system_prompt(
    config: &loopal_config::ResolvedConfig,
    kernel: &loopal_kernel::Kernel,
    cwd: &std::path::Path,
    options: SessionPromptOptions<'_>,
) -> String {
    let skills: Vec<_> = config
        .skills
        .values()
        .map(|entry| entry.skill.clone())
        .collect();
    let skills_summary = loopal_config::format_skills_summary(&skills);
    let translation_view = if let Some(store) = config.secrets.as_ref() {
        let names = store.list_names().await;
        Some(loopal_secret_runtime::TranslationView::from_names(names))
    } else {
        None
    };
    let (instructions, _) =
        loopal_secret_runtime::translate_outbound(&config.instructions, translation_view.as_ref());
    let (memory, _) =
        loopal_secret_runtime::translate_outbound(&config.memory, translation_view.as_ref());
    let mut prompt = loopal_context::system_prompt::build_system_prompt(
        &instructions,
        options.tool_defs,
        options.mode,
        &cwd.to_string_lossy(),
        &skills_summary,
        &memory,
        options.agent_type,
        options.features,
        options.depth,
    );
    crate::prompt_post::append_runtime_sections(&mut prompt, kernel).await;
    prompt
}

#[cfg(test)]
#[path = "agent_setup_prompt_tests.rs"]
mod tests;
