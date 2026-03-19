use serde::{Deserialize, Serialize};

/// Permission level required by a tool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// Read-only operations (e.g., Read, Glob, Grep, Ls)
    ReadOnly,
    /// Supervised operations requiring approval (e.g., Write, Edit)
    Supervised,
    /// Dangerous operations (e.g., Bash, destructive commands)
    Dangerous,
}

/// Permission mode set by user.
///
/// Two modes only — controls user interaction policy.
/// Sandbox enforcement is a separate, orthogonal layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// All tools auto-allowed, no approval needed.
    /// Sandbox still blocks dangerous operations.
    Bypass,
    /// ReadOnly auto-allowed; Supervised and Dangerous require human approval.
    /// Sandbox checks run first; denied operations never reach the user prompt.
    Supervised,
}

/// Decision from permission check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    /// Automatically allowed
    Allow,
    /// Requires user confirmation
    Ask,
    /// Denied
    Deny,
}

impl PermissionMode {
    pub fn check(&self, level: PermissionLevel) -> PermissionDecision {
        match self {
            PermissionMode::Bypass => PermissionDecision::Allow,
            PermissionMode::Supervised => match level {
                PermissionLevel::ReadOnly => PermissionDecision::Allow,
                _ => PermissionDecision::Ask,
            },
        }
    }
}
