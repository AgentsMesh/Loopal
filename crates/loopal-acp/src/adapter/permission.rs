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
    pub(crate) async fn handle_permission_request(
        &self,
        agent_name: String,
        interaction_id: String,
        tool_name: String,
        tool_input: Value,
        intent_digest: Option<loopal_protocol::PermissionIntentDigest>,
        session_id: &str,
    ) {
        let tool_call = ToolCallUpdate::new(
            ToolCallId::new(interaction_id.as_str()),
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

        let outcome = match self
            .acp_out
            .request("session/request_permission", acp_params)
            .await
        {
            Ok(value) => parse_permission_outcome(&value),
            Err(e) => {
                warn!("permission request to IDE failed: {e}");
                PermissionSelection::DENY
            }
        };

        self.client
            .respond_permission_with_memory(
                &agent_name,
                &interaction_id,
                intent_digest,
                outcome.allow,
                outcome.remember_session,
            )
            .await;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PermissionSelection {
    allow: bool,
    remember_session: bool,
}

impl PermissionSelection {
    const DENY: Self = Self {
        allow: false,
        remember_session: false,
    };
}

fn parse_permission_outcome(value: &Value) -> PermissionSelection {
    if let Ok(resp) = serde_json::from_value::<RequestPermissionResponse>(value.clone()) {
        return match resp.outcome {
            RequestPermissionOutcome::Selected(selected) => match selected.option_id.0.as_ref() {
                "allow_once" => PermissionSelection {
                    allow: true,
                    remember_session: false,
                },
                "allow_always" => PermissionSelection {
                    allow: true,
                    remember_session: true,
                },
                _ => PermissionSelection::DENY,
            },
            _ => PermissionSelection::DENY,
        };
    }
    if value.get("outcome").and_then(Value::as_str) == Some("allow") {
        PermissionSelection {
            allow: true,
            remember_session: false,
        }
    } else {
        PermissionSelection::DENY
    }
}

#[cfg(test)]
#[path = "permission/tests.rs"]
mod tests;
