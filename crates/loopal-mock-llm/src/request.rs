use serde::Serialize;
use serde_json::Value;

use crate::protocol::WireProtocol;

#[derive(Default)]
pub(crate) struct CanonicalRequest {
    pub model: String,
    pub message_count: usize,
    pub tool_names: Vec<String>,
    pub tool_result_ids: Vec<String>,
    pub tool_result_error_ids: Vec<String>,
    pub assistant_block_types: Vec<String>,
    pub server_block_count: usize,
    pub image_block_count: usize,
    pub last_user_text: String,
    pub has_system: bool,
    pub system_text: String,
    pub thinking_enabled: bool,
    pub stream: bool,
    pub max_tokens: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRecord {
    pub sequence: usize,
    pub protocol: String,
    pub model: String,
    pub message_count: usize,
    pub tool_count: usize,
    pub tool_names: Vec<String>,
    pub tool_result_ids: Vec<String>,
    pub tool_result_error_ids: Vec<String>,
    pub assistant_block_types: Vec<String>,
    pub server_block_count: usize,
    pub image_block_count: usize,
    pub last_user_text: String,
    pub has_system: bool,
    pub system_text: String,
    pub thinking_enabled: bool,
    pub stream: bool,
    pub max_tokens: u64,
    pub api_key_present: bool,
    pub protocol_version_present: bool,
    pub matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_label: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub match_errors: Vec<String>,
}

impl RequestRecord {
    pub(crate) fn from_body(
        sequence: usize,
        protocol: WireProtocol,
        body: &Value,
        route_model: Option<&str>,
        key: bool,
        version: bool,
    ) -> Self {
        let value = crate::request_wire::canonicalize(protocol, body, route_model);
        Self {
            sequence,
            protocol: protocol.as_str().into(),
            model: value.model,
            message_count: value.message_count,
            tool_count: value.tool_names.len(),
            tool_names: value.tool_names,
            tool_result_ids: value.tool_result_ids,
            tool_result_error_ids: value.tool_result_error_ids,
            assistant_block_types: value.assistant_block_types,
            server_block_count: value.server_block_count,
            image_block_count: value.image_block_count,
            last_user_text: value.last_user_text,
            has_system: value.has_system,
            system_text: value.system_text,
            thinking_enabled: value.thinking_enabled,
            stream: value.stream,
            max_tokens: value.max_tokens,
            api_key_present: key,
            protocol_version_present: version,
            matched: true,
            call_label: None,
            match_errors: Vec::new(),
        }
    }
}
