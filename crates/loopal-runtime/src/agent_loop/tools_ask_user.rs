use loopal_message::ContentBlock;

use super::question_format::format_response;
use super::question_parse::parse_questions;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn handle_ask_user(
        &mut self,
        _turn_ctx: &mut TurnContext,
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
        Ok((idx, self.emit_and_block(id, name, content, is_error, None).await?))
    }

    async fn handle_ask_user_schema_err(
        &mut self,
        idx: usize,
        id: &str,
        name: &str,
        err_msg: String,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        Ok((idx, self.emit_and_block(id, name, err_msg, true, None).await?))
    }
}
