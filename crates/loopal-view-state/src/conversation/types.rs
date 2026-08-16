use loopal_protocol::{MessageSource, PermissionIntentDigest, SkillInvocation};
use loopal_tool_invocation::ToolInvocation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolInvocation>,
    pub image_count: usize,
    pub skill_info: Option<SkillInvocation>,
    pub inbox: Option<InboxOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ui_local: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxOrigin {
    pub message_id: String,
    pub source: MessageSource,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionChoice {
    #[default]
    Allow,
    Deny,
}

impl PermissionChoice {
    pub fn toggle(self) -> Self {
        match self {
            Self::Allow => Self::Deny,
            Self::Deny => Self::Allow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPermission {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_digest: Option<PermissionIntentDigest>,
    #[serde(default)]
    pub cursor: PermissionChoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPlanApproval {
    pub id: String,
    pub plan_content: String,
    pub plan_path: String,
}
