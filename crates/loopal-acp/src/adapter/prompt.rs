//! ACP prompt + cancel handlers.

use agent_client_protocol_schema::{ContentBlock, PromptRequest};
use loopal_protocol::{Envelope, MessageSource};
use serde_json::Value;

use crate::adapter::AcpAdapter;
use crate::jsonrpc;
use crate::types::make_prompt_response;

impl AcpAdapter {
    /// Handle `session/prompt`: route user message through Hub, run event loop.
    pub(crate) async fn handle_prompt(&self, id: i64, params: Value) {
        let req: PromptRequest = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
                return;
            }
        };

        let session_id = self.session_id.lock().await.clone();
        let Some(ref sid) = session_id else {
            self.acp_out
                .respond_error(id, jsonrpc::INVALID_REQUEST, "no session")
                .await;
            return;
        };
        if sid.as_str() != req.session_id.0.as_ref() {
            self.acp_out
                .respond_error(id, jsonrpc::INVALID_REQUEST, "session mismatch")
                .await;
            return;
        }

        let text: String = req
            .prompt
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text(tc) => Some(tc.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let envelope = Envelope::new(MessageSource::Human, "main", text);
        if let Err(e) = self.client.route_envelope(&envelope).await {
            self.acp_out
                .respond_error(id, jsonrpc::INTERNAL_ERROR, &e)
                .await;
            return;
        }

        let stop_reason = self.run_event_loop(sid).await;
        let result = make_prompt_response(stop_reason);
        self.acp_out
            .respond(id, serde_json::to_value(result).unwrap_or_default())
            .await;
    }

    /// Handle `session/cancel` as a request (backward compat).
    pub(crate) async fn handle_cancel_request(&self, id: i64) {
        if self.session_id.lock().await.is_none() {
            self.acp_out
                .respond_error(id, jsonrpc::INVALID_REQUEST, "no active session")
                .await;
            return;
        }
        self.client.interrupt().await;
        self.acp_out.respond(id, Value::Null).await;
    }

    /// Handle `session/cancel` as a notification (ACP spec).
    pub(crate) async fn handle_cancel_notification(&self) {
        if self.session_id.lock().await.is_some() {
            self.client.interrupt().await;
        }
    }
}
