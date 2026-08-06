use serde::{Deserialize, Serialize};

/// Interactive request types a UI may explicitly handle.
///
/// Registration is fail-closed: omitted capabilities deserialize to `false`.
/// Adding another interactive surface requires adding a field here and a match
/// arm in [`UiCapabilities::supports`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCapability {
    Permission,
    Question,
    PlanApproval,
}

/// Capabilities declared by a UI client during `hub/register`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCapabilities {
    #[serde(default)]
    pub permission: bool,
    #[serde(default)]
    pub question: bool,
    #[serde(default)]
    pub plan_approval: bool,
}

impl UiCapabilities {
    pub const NONE: Self = Self {
        permission: false,
        question: false,
        plan_approval: false,
    };

    pub const ALL: Self = Self {
        permission: true,
        question: true,
        plan_approval: true,
    };

    pub const fn supports(self, capability: UiCapability) -> bool {
        match capability {
            UiCapability::Permission => self.permission,
            UiCapability::Question => self.question,
            UiCapability::PlanApproval => self.plan_approval,
        }
    }
}
