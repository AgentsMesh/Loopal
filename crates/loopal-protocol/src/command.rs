use serde::{Deserialize, Serialize};

/// Agent operating mode, mirrored here in types to avoid circular dependency
/// with loopal-runtime.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    #[default]
    Act,
    Plan,
}

impl AgentMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentMode::Act => "act",
            AgentMode::Plan => "plan",
        }
    }
}
