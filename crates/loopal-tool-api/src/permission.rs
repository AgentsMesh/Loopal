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
/// Three modes: Bypass (trust everything), Auto (LLM classifies danger),
/// Supervised (human approves everything non-readonly).
/// Sandbox enforcement is a separate, orthogonal layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// All tools auto-allowed, no approval needed.
    /// Sandbox still blocks dangerous operations.
    Bypass,
    /// ReadOnly auto-allowed; Supervised and Dangerous require human approval.
    Supervised,
    /// ReadOnly + Supervised auto-allowed; Dangerous goes to LLM classifier.
    /// Falls back to human approval when classifier is unavailable or degraded.
    Auto,
}

/// Decision from permission check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    /// Automatically allowed
    Allow,
    /// Requires user confirmation (or classifier in Auto mode)
    Ask,
    /// Denied
    Deny,
}

impl PermissionMode {
    pub fn check(&self, level: PermissionLevel) -> PermissionDecision {
        match self {
            PermissionMode::Bypass => PermissionDecision::Allow,
            PermissionMode::Auto => match level {
                PermissionLevel::ReadOnly | PermissionLevel::Supervised => {
                    PermissionDecision::Allow
                }
                PermissionLevel::Dangerous => PermissionDecision::Ask,
            },
            PermissionMode::Supervised => match level {
                PermissionLevel::ReadOnly => PermissionDecision::Allow,
                _ => PermissionDecision::Ask,
            },
        }
    }
}
