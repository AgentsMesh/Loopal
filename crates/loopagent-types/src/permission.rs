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

/// Permission mode set by user
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// Default: auto-allow ReadOnly, prompt for Supervised+Dangerous
    Default,
    /// Accept edits: auto-allow ReadOnly+Supervised, prompt for Dangerous
    AcceptEdits,
    /// Bypass all permission checks
    BypassPermissions,
    /// Plan mode: only allow ReadOnly, deny everything else
    Plan,
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
        match (self, level) {
            // Plan mode: only read
            (PermissionMode::Plan, PermissionLevel::ReadOnly) => PermissionDecision::Allow,
            (PermissionMode::Plan, _) => PermissionDecision::Deny,

            // Bypass: allow all
            (PermissionMode::BypassPermissions, _) => PermissionDecision::Allow,

            // AcceptEdits: auto-allow read + supervised
            (PermissionMode::AcceptEdits, PermissionLevel::ReadOnly) => PermissionDecision::Allow,
            (PermissionMode::AcceptEdits, PermissionLevel::Supervised) => {
                PermissionDecision::Allow
            }
            (PermissionMode::AcceptEdits, PermissionLevel::Dangerous) => PermissionDecision::Ask,

            // Default: auto-allow read, ask for rest
            (PermissionMode::Default, PermissionLevel::ReadOnly) => PermissionDecision::Allow,
            (PermissionMode::Default, _) => PermissionDecision::Ask,
        }
    }
}
