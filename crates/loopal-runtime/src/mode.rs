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
    pub fn system_prompt_suffix(&self) -> &str {
        match self {
            AgentMode::Act => "",
            AgentMode::Plan => {
                "\n\nYou are in PLAN mode. Explore the codebase and design a solution. \
                 Write your plan to .loopal/plans/plan.md. Use AskUser to confirm with \
                 the user before calling ExitPlanMode."
            }
        }
    }
}
