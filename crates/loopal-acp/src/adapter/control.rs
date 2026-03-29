//! ACP control handlers: session/set_mode, session/set_config_option.

use loopal_protocol::ControlCommand;
use serde_json::Value;

use crate::adapter::AcpAdapter;
use crate::jsonrpc;

impl AcpAdapter {
    /// Handle `session/set_mode` → HubClient.send_control(ModeSwitch).
    pub(crate) async fn handle_set_mode(&self, id: i64, params: Value) {
        let mode_id = params["modeId"].as_str().unwrap_or("");
        let mode = match mode_id {
            "plan" => loopal_protocol::AgentMode::Plan,
            "act" => loopal_protocol::AgentMode::Act,
            _ => {
                self.acp_out
                    .respond_error(
                        id,
                        jsonrpc::INVALID_REQUEST,
                        &format!("unknown modeId: {mode_id}"),
                    )
                    .await;
                return;
            }
        };

        match self
            .client
            .send_control(&ControlCommand::ModeSwitch(mode))
            .await
        {
            Ok(_) => self.acp_out.respond(id, serde_json::json!({})).await,
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INTERNAL_ERROR, &e)
                    .await;
            }
        }
    }

    /// Handle `session/set_config_option` → HubClient.send_control(ModelSwitch|ThinkingSwitch).
    pub(crate) async fn handle_set_config_option(&self, id: i64, params: Value) {
        let config_id = params["configId"].as_str().unwrap_or("");
        let value = params["value"].as_str().unwrap_or("").to_string();

        let cmd = match config_id {
            "model" => ControlCommand::ModelSwitch(value),
            "thinking" => ControlCommand::ThinkingSwitch(value),
            _ => {
                self.acp_out
                    .respond_error(
                        id,
                        jsonrpc::INVALID_REQUEST,
                        &format!("unknown configId: {config_id}"),
                    )
                    .await;
                return;
            }
        };

        match self.client.send_control(&cmd).await {
            Ok(_) => {
                self.acp_out
                    .respond(id, serde_json::json!({"configOptions": []}))
                    .await;
            }
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INTERNAL_ERROR, &e)
                    .await;
            }
        }
    }
}
