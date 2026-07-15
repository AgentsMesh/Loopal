use serde::{Deserialize, Serialize};

use super::super::Method;

pub const DESKTOP_LIST_SKILLS: Method = Method {
    name: "desktop/listSkills",
};
pub const DESKTOP_GET_SKILL: Method = Method {
    name: "desktop/getSkill",
};
pub const DESKTOP_UPSERT_SKILL: Method = Method {
    name: "desktop/upsertSkill",
};
pub const DESKTOP_DELETE_SKILL: Method = Method {
    name: "desktop/deleteSkill",
};
pub const DESKTOP_LIST_PLUGINS: Method = Method {
    name: "desktop/listPlugins",
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopListSkillsParams {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopGetSkillParams {
    pub workspace_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopUpsertSkillParams {
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub body: String,
    #[serde(default)]
    pub expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopDeleteSkillParams {
    pub workspace_id: String,
    pub name: String,
    pub expected_revision: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DesktopSkillScope {
    Global,
    Project,
    Plugin,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSkillDefinition {
    pub name: String,
    pub description: String,
    pub has_arguments: bool,
    pub source: String,
    pub scope: DesktopSkillScope,
    pub editable: bool,
    pub effective: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSkillsResponse {
    pub workspace_id: String,
    pub skills: Vec<DesktopSkillDefinition>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSkillDetail {
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub has_arguments: bool,
    pub source: String,
    pub scope: DesktopSkillScope,
    pub editable: bool,
    pub effective: bool,
    pub body: String,
    pub revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPluginSummary {
    pub name: String,
    pub source: String,
    pub skills: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub hook_count: usize,
    pub has_instructions: bool,
    pub has_memory: bool,
    pub has_settings: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPluginsResponse {
    pub workspace_id: String,
    pub plugins: Vec<DesktopPluginSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_inputs_are_strict_and_camel_case() {
        let input: DesktopUpsertSkillParams = serde_json::from_value(serde_json::json!({
            "workspaceId": "workspace", "name": "/check", "description": "Check",
            "body": "Body", "expectedRevision": "a".repeat(64)
        }))
        .unwrap();
        assert_eq!(input.workspace_id, "workspace");
        assert_eq!(input.expected_revision.unwrap().len(), 64);
        assert!(
            serde_json::from_value::<DesktopGetSkillParams>(serde_json::json!({
                "workspaceId": "workspace", "name": "/check", "extra": true
            }))
            .is_err()
        );
    }

    #[test]
    fn response_shape_matches_desktop_contract() {
        let value = serde_json::to_value(DesktopSkillDefinition {
            name: "/check".into(),
            description: "Check".into(),
            has_arguments: true,
            source: "global".into(),
            scope: DesktopSkillScope::Global,
            editable: true,
            effective: true,
            revision: Some("a".repeat(64)),
        })
        .unwrap();
        assert_eq!(value["hasArguments"], true);
        assert_eq!(value["scope"], "global");
        assert!(value.get("revision").is_some());
    }
}
