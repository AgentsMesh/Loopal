use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;

use super::question_format::format_response;
use super::question_parse::parse_questions;
use super::runner::AgentLoopRunner;
use super::tools_inject::{error_block, success_block};

impl AgentLoopRunner {
    pub(super) async fn handle_ask_user(
        &mut self,
        idx: usize,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        match parse_questions(input) {
            Ok(questions) => self.handle_ask_user_ok(idx, id, name, questions).await,
            Err(err_msg) => {
                self.handle_ask_user_schema_err(idx, id, name, err_msg)
                    .await
            }
        }
    }

    async fn handle_ask_user_ok(
        &mut self,
        idx: usize,
        id: &str,
        name: &str,
        questions: Vec<loopal_protocol::Question>,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        self.refresh_decision_context().await;
        let response = self.params.deps.frontend.ask_user(questions.clone()).await;
        let (content, is_error) = format_response(&response, &questions);
        self.emit(AgentEventPayload::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            result: content.clone(),
            is_error,
            duration_ms: None,
            metadata: None,
        })
        .await?;
        // reason: frontend response (cancelled/unsupported) is a completed
        // answer, not a retryable error — `is_error` flags UI only. Only schema
        // failures (handle_ask_user_schema_err) emit error_block for LLM retry.
        Ok((idx, success_block(id, &content)))
    }

    async fn handle_ask_user_schema_err(
        &mut self,
        idx: usize,
        id: &str,
        name: &str,
        err_msg: String,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        self.emit(AgentEventPayload::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            result: err_msg.clone(),
            is_error: true,
            duration_ms: None,
            metadata: None,
        })
        .await?;
        Ok((idx, error_block(id, &err_msg)))
    }
}
