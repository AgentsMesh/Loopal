//! ACP (Agent Client Protocol) message types.
//!
//! Only the subset required by Loopal is defined here:
//! initialize, session lifecycle, prompt, permission, and session updates.
//!
//! Protocol completeness: some variants/fields are not yet used internally
//! but are part of the ACP spec.
#![allow(dead_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Initialize ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    #[serde(default)]
    pub client_capabilities: serde_json::Value,
    pub client_info: Option<ClientInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ClientInfo {
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub agent_capabilities: AgentCapabilities,
    pub agent_info: AgentInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    pub streaming: bool,
}

#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub name: String,
    pub version: String,
}

// ── Session ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct NewSessionParams {
    #[serde(default = "default_cwd")]
    pub cwd: PathBuf,
}

fn default_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionResult {
    pub session_id: String,
}

// ── Prompt ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptParams {
    pub session_id: String,
    pub prompt: Vec<AcpContentBlock>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResult {
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AcpContentBlock {
    Text { text: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Cancelled,
}

// ── Session Update (server → client notifications) ──────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
    pub session_id: String,
    pub update: SessionUpdate,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionUpdate {
    AgentMessageChunk {
        message_id: String,
        content: Vec<AcpContentBlock>,
    },
    ToolCall {
        tool_call_id: String,
        title: String,
        tool_call_kind: ToolKind,
        status: ToolCallStatus,
    },
    ToolCallUpdate {
        tool_call_id: String,
        status: ToolCallStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
}

// ── Permission (server → client request) ────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionParams {
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct RequestPermissionResult {
    pub outcome: PermissionOutcome,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOutcome {
    Allow,
    Deny,
}

// ── Enums ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Edit,
    Delete,
    Execute,
    Search,
    Fetch,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}
