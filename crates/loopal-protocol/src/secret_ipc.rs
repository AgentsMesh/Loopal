use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretCaller {
    pub agent_name: String,
    pub depth: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretGetRequest {
    pub cwd: String,
    pub name: String,
    pub caller: SecretCaller,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretGetResponse {
    pub plaintext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretListNamesRequest {
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretListNamesResponse {
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretHealthRequest {
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretHealthResponse {
    pub vault_count: u32,
    pub default_vault: String,
    pub last_op_ts: u64,
}

/// Structured Hub-side error returned by `hub/secret/*` handlers. Serialized
/// as JSON and carried in the IPC `RpcError::Remote { message }`. Client side
/// decodes this back into `SecretError` instead of fragile string matching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretIpcError {
    SecretNotFound { name: String },
    VaultNotFound { cwd: String },
    PermissionDenied,
    DecryptFailed { detail: String },
    InvalidName { name: String },
    TemplateParse { detail: String },
    Ipc { detail: String },
}
