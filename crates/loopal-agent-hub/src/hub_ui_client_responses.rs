use loopal_ipc::protocol::methods;
use tracing::warn;

use crate::hub_ui_client::HubClient;

#[cfg(not(test))]
const RESPONSE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(test)]
const RESPONSE_DEADLINE: std::time::Duration = std::time::Duration::from_millis(50);

impl HubClient {
    pub async fn respond_permission(&self, agent_name: &str, interaction_id: &str, allow: bool) {
        let params = serde_json::json!({
            "agent_name": agent_name,
            // Wire key retained for compatibility; value is the opaque Hub token.
            "tool_call_id": interaction_id,
            "allow": allow,
        });
        if let Err(e) = self
            .send_interaction_response(methods::HUB_PERMISSION_RESPONSE.name, params)
            .await
        {
            warn!(
                agent_name,
                interaction_id, "hub/permission_response failed: {e}"
            );
        }
    }

    pub async fn respond_question(
        &self,
        agent_name: &str,
        interaction_id: &str,
        answers: Vec<String>,
    ) {
        debug_assert!(
            !interaction_id.is_empty(),
            "respond_question requires non-empty interaction_id"
        );
        let response = loopal_protocol::UserQuestionResponse::answered(interaction_id, answers);
        let params = serde_json::json!({
            "agent_name": agent_name,
            // Wire key retained for compatibility; value is the opaque Hub token.
            "question_id": interaction_id,
            "response": response,
        });
        if let Err(e) = self
            .send_interaction_response(methods::HUB_QUESTION_RESPONSE.name, params)
            .await
        {
            warn!(
                agent_name,
                interaction_id, "hub/question_response failed: {e}"
            );
        }
    }

    pub async fn cancel_question(&self, agent_name: &str, interaction_id: &str) {
        debug_assert!(
            !interaction_id.is_empty(),
            "cancel_question requires non-empty interaction_id"
        );
        let response = loopal_protocol::UserQuestionResponse::cancelled(interaction_id);
        let params = serde_json::json!({
            "agent_name": agent_name,
            // Wire key retained for compatibility; value is the opaque Hub token.
            "question_id": interaction_id,
            "response": response,
        });
        if let Err(e) = self
            .send_interaction_response(methods::HUB_QUESTION_RESPONSE.name, params)
            .await
        {
            warn!(
                agent_name,
                interaction_id, "hub/question_cancel failed: {e}"
            );
        }
    }

    pub async fn respond_plan_approval(
        &self,
        agent_name: &str,
        interaction_id: &str,
        decision: &str,
        edited_plan: Option<&str>,
    ) {
        let params = serde_json::json!({
            "agent_name": agent_name,
            // Wire key retained for compatibility; value is the opaque Hub token.
            "request_id": interaction_id,
            "decision": decision,
            "edited_plan": edited_plan,
        });
        if let Err(e) = self
            .send_interaction_response(methods::HUB_PLAN_APPROVAL_RESPONSE.name, params)
            .await
        {
            warn!(
                agent_name,
                interaction_id, "hub/plan_approval_response failed: {e}"
            );
        }
    }

    async fn send_interaction_response(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), String> {
        tokio::time::timeout(
            RESPONSE_DEADLINE,
            self.connection().send_request(method, params),
        )
        .await
        .map_err(|_| format!("{method} timed out after {RESPONSE_DEADLINE:?}"))?
        .map(|_| ())
        .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "hub_ui_client_responses/tests.rs"]
mod tests;
