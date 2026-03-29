//! ACP permission and question request handlers.

use agent_client_protocol_schema::{
    PermissionOption, PermissionOptionId, PermissionOptionKind, RequestPermissionOutcome,
    RequestPermissionResponse, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};
use serde_json::Value;
use tracing::warn;

use crate::adapter::AcpAdapter;
use crate::translate::map_tool_kind;

impl AcpAdapter {
    /// Forward agent/permission request to IDE as session/request_permission.
    ///
    /// Uses full ACP `PermissionOption` model with allow_once/always + reject_once/always.
    pub(crate) async fn handle_permission_request(
        &self,
        request_id: i64,
        params: Value,
        session_id: &str,
    ) {
        let tool_call_id = params["tool_call_id"].as_str().unwrap_or("").to_string();
        let tool_name = params["tool_name"].as_str().unwrap_or("").to_string();
        let tool_input = params.get("tool_input").cloned().unwrap_or(Value::Null);

        let tool_call = ToolCallUpdate::new(
            ToolCallId::new(tool_call_id.as_str()),
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::Pending)
                .title(tool_name.clone())
                .kind(map_tool_kind(&tool_name))
                .raw_input(tool_input),
        );

        let options = vec![
            PermissionOption::new(
                PermissionOptionId::new("allow_once"),
                "Allow once",
                PermissionOptionKind::AllowOnce,
            ),
            PermissionOption::new(
                PermissionOptionId::new("allow_always"),
                "Always allow",
                PermissionOptionKind::AllowAlways,
            ),
            PermissionOption::new(
                PermissionOptionId::new("reject_once"),
                "Reject once",
                PermissionOptionKind::RejectOnce,
            ),
            PermissionOption::new(
                PermissionOptionId::new("reject_always"),
                "Always reject",
                PermissionOptionKind::RejectAlways,
            ),
        ];

        let acp_req = agent_client_protocol_schema::RequestPermissionRequest::new(
            session_id.to_string(),
            tool_call,
            options,
        );
        let acp_params = serde_json::to_value(acp_req).unwrap_or_default();

        let allow = match self
            .acp_out
            .request("session/request_permission", acp_params)
            .await
        {
            Ok(value) => parse_permission_outcome(&value),
            Err(e) => {
                warn!("permission request to IDE failed: {e}");
                false
            }
        };

        self.client.respond_permission(request_id, allow).await;
    }

    /// Forward agent/question as `_loopal/question` extension request.
    /// Falls back to auto-respond if IDE doesn't support it.
    pub(crate) async fn handle_question_request(&self, request_id: i64, params: Value) {
        let ext_params = serde_json::json!({
            "questions": params.get("questions").cloned().unwrap_or(Value::Null),
        });

        let answers = match self.acp_out.request("_loopal/question", ext_params).await {
            Ok(value) => value["answers"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| vec!["(no answer)".into()]),
            Err(_) => vec!["(not supported in ACP mode)".into()],
        };

        self.client.respond_question(request_id, answers).await;
    }
}

/// Parse a `RequestPermissionResponse` to determine allow/deny.
fn parse_permission_outcome(value: &Value) -> bool {
    // Try official ACP format first
    if let Ok(resp) = serde_json::from_value::<RequestPermissionResponse>(value.clone()) {
        return match resp.outcome {
            RequestPermissionOutcome::Selected(sel) => {
                let oid = sel.option_id.0.as_ref();
                oid.starts_with("allow")
            }
            RequestPermissionOutcome::Cancelled => false,
            _ => false, // non_exhaustive fallback
        };
    }
    // Legacy format: {"outcome": "allow"}
    value
        .get("outcome")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "allow")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn acp_allow_once() {
        let v = json!({"outcome": {"outcome": "selected", "optionId": "allow_once"}});
        assert!(parse_permission_outcome(&v));
    }

    #[test]
    fn acp_allow_always() {
        let v = json!({"outcome": {"outcome": "selected", "optionId": "allow_always"}});
        assert!(parse_permission_outcome(&v));
    }

    #[test]
    fn acp_reject_once() {
        let v = json!({"outcome": {"outcome": "selected", "optionId": "reject_once"}});
        assert!(!parse_permission_outcome(&v));
    }

    #[test]
    fn acp_reject_always() {
        let v = json!({"outcome": {"outcome": "selected", "optionId": "reject_always"}});
        assert!(!parse_permission_outcome(&v));
    }

    #[test]
    fn acp_cancelled() {
        let v = json!({"outcome": {"outcome": "cancelled"}});
        assert!(!parse_permission_outcome(&v));
    }

    #[test]
    fn legacy_allow() {
        let v = json!({"outcome": "allow"});
        assert!(parse_permission_outcome(&v));
    }

    #[test]
    fn legacy_deny() {
        let v = json!({"outcome": "deny"});
        assert!(!parse_permission_outcome(&v));
    }

    #[test]
    fn malformed_returns_false() {
        assert!(!parse_permission_outcome(&json!({})));
        assert!(!parse_permission_outcome(&json!(null)));
        assert!(!parse_permission_outcome(&json!(42)));
    }
}
