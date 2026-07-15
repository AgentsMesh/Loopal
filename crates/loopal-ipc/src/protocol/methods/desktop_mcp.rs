use serde::{Deserialize, Serialize};

use super::super::Method;

pub const DESKTOP_LIST_MCP_SERVERS: Method = Method {
    name: "desktop/listMcpServers",
};
pub const DESKTOP_UPSERT_MCP_SERVER: Method = Method {
    name: "desktop/upsertMcpServer",
};
pub const DESKTOP_DELETE_MCP_SERVER: Method = Method {
    name: "desktop/deleteMcpServer",
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopListMcpServersParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopMcpSharing {
    HubSingleton,
    PerAgent,
    SpawnTree,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopMcpCwdIsolation {
    pub arg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_subdir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopMcpSecretTarget {
    Env,
    Header,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopMcpSecretOperation {
    Set,
    Remove,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopMcpSecretPatch {
    pub target: DesktopMcpSecretTarget,
    pub name: String,
    pub operation: DesktopMcpSecretOperation,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DesktopMcpServerInput {
    Stdio {
        name: String,
        command: String,
        #[serde(default)]
        args: Vec<String>,
        enabled: bool,
        timeout_ms: u64,
        sharing: DesktopMcpSharing,
        #[serde(default)]
        cwd_isolation: Option<DesktopMcpCwdIsolation>,
        #[serde(default)]
        secret_patches: Vec<DesktopMcpSecretPatch>,
    },
    StreamableHttp {
        name: String,
        url: String,
        enabled: bool,
        timeout_ms: u64,
        sharing: DesktopMcpSharing,
        #[serde(default)]
        secret_patches: Vec<DesktopMcpSecretPatch>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopUpsertMcpServerParams {
    pub workspace_id: String,
    pub server: DesktopMcpServerInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopDeleteMcpServerParams {
    pub workspace_id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopMcpSecretStatus {
    pub name: String,
    pub configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum DesktopMcpServerDefinition {
    Stdio {
        name: String,
        source: String,
        command: String,
        args: Vec<String>,
        enabled: bool,
        timeout_ms: u64,
        sharing: DesktopMcpSharing,
        cwd_isolation: Option<DesktopMcpCwdIsolation>,
        env: Vec<DesktopMcpSecretStatus>,
    },
    StreamableHttp {
        name: String,
        source: String,
        url: String,
        enabled: bool,
        timeout_ms: u64,
        sharing: DesktopMcpSharing,
        headers: Vec<DesktopMcpSecretStatus>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopMcpServersResponse {
    pub workspace_id: String,
    pub servers: Vec<DesktopMcpServerDefinition>,
}
