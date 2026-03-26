use serde::{Deserialize, Serialize};

/// Agent operating mode, mirrored here in types to avoid circular dependency
/// with loopal-runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    Act,
    Plan,
}
