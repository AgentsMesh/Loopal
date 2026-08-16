use loopal_protocol::Question;
use serde_json::Value;

use crate::adapter::AcpAdapter;

impl AcpAdapter {
    pub(crate) async fn handle_question_request(
        &self,
        agent_name: String,
        interaction_id: String,
        questions: Vec<Question>,
    ) {
        let ext_params = serde_json::json!({
            "questions": serde_json::to_value(&questions).unwrap_or(Value::Null),
        });
        let answers = match self.acp_out.request("_loopal/question", ext_params).await {
            Ok(value) => value["answers"]
                .as_array()
                .map(|answers| {
                    answers
                        .iter()
                        .filter_map(|answer| answer.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        if answers.is_empty() {
            self.client
                .cancel_question(&agent_name, &interaction_id)
                .await;
        } else {
            self.client
                .respond_question(&agent_name, &interaction_id, answers)
                .await;
        }
    }
}
