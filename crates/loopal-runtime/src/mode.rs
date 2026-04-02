use loopal_protocol::AgentMode as TypesAgentMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Act,
    Plan,
}

impl From<TypesAgentMode> for AgentMode {
    fn from(mode: TypesAgentMode) -> Self {
        match mode {
            TypesAgentMode::Act => AgentMode::Act,
            TypesAgentMode::Plan => AgentMode::Plan,
        }
    }
}

impl AgentMode {
    /// Append mode-specific instructions after the system prompt.
    ///
    /// Plan mode returns empty — the `plan-5phase` Fragment handles all
    /// plan instructions, and `handle_enter_plan` injects the plan file
    /// path via `tool_result`. Keeping the suffix empty avoids conflicting
    /// with the Fragment's 5-phase workflow.
    pub fn system_prompt_suffix(&self) -> &str {
        match self {
            AgentMode::Act => "",
            AgentMode::Plan => "",
        }
    }
}
