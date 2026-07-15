use loopal_ipc::protocol::methods;
use tracing::warn;

use crate::hub_ui_client::HubClient;

impl HubClient {
    pub async fn respond_permission(&self, agent_name: &str, tool_call_id: &str, allow: bool) {
        let params = serde_json::json!({
            "agent_name": agent_name,
            "tool_call_id": tool_call_id,
            "allow": allow,
        });
        if let Err(e) = self
            .connection()
            .send_request(methods::HUB_PERMISSION_RESPONSE.name, params)
            .await
        {
            warn!(
                agent_name,
                tool_call_id, "hub/permission_response failed: {e}"
            );
        }
    }

    pub async fn respond_question(
        &self,
        agent_name: &str,
        question_id: &str,
        answers: Vec<String>,
    ) {
        debug_assert!(
            !question_id.is_empty(),
            "respond_question requires non-empty question_id"
        );
        let response = loopal_protocol::UserQuestionResponse::answered(question_id, answers);
        let params = serde_json::json!({
            "agent_name": agent_name,
            "question_id": question_id,
            "response": response,
        });
        if let Err(e) = self
            .connection()
            .send_request(methods::HUB_QUESTION_RESPONSE.name, params)
            .await
        {
            warn!(agent_name, question_id, "hub/question_response failed: {e}");
        }
    }

    pub async fn cancel_question(&self, agent_name: &str, question_id: &str) {
        debug_assert!(
            !question_id.is_empty(),
            "cancel_question requires non-empty question_id"
        );
        let response = loopal_protocol::UserQuestionResponse::cancelled(question_id);
        let params = serde_json::json!({
            "agent_name": agent_name,
            "question_id": question_id,
            "response": response,
        });
        if let Err(e) = self
            .connection()
            .send_request(methods::HUB_QUESTION_RESPONSE.name, params)
            .await
        {
            warn!(agent_name, question_id, "hub/question_cancel failed: {e}");
        }
    }

    pub async fn respond_plan_approval(
        &self,
        agent_name: &str,
        request_id: &str,
        decision: &str,
        edited_plan: Option<&str>,
    ) {
        let params = serde_json::json!({
            "agent_name": agent_name,
            "request_id": request_id,
            "decision": decision,
            "edited_plan": edited_plan,
        });
        if let Err(e) = self
            .connection()
            .send_request(methods::HUB_PLAN_APPROVAL_RESPONSE.name, params)
            .await
        {
            warn!(
                agent_name,
                request_id, "hub/plan_approval_response failed: {e}"
            );
        }
    }
}
