use std::time::Instant;

use loopal_message::Message;
use loopal_protocol::Question;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};
use tracing::{info, warn};

use crate::ClassifierEngine;
use crate::question_prompt;
use futures::StreamExt;

// reason: `@`-prefix lies outside the tool-name namespace (PascalCase identifiers),
// so question-side breaker accounting never collides with a real tool's counter.
const QUESTION_BREAKER_KEY: &str = "@question";

pub struct QuestionResult {
    pub answers: Vec<Vec<String>>,
    pub reason: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl QuestionResult {
    pub fn ok(answers: Vec<Vec<String>>, reason: String, duration_ms: u64) -> Self {
        Self {
            answers,
            reason,
            duration_ms,
            error: None,
        }
    }

    pub fn error(duration_ms: u64, error: impl Into<String>) -> Self {
        Self {
            answers: Vec::new(),
            reason: String::new(),
            duration_ms,
            error: Some(error.into()),
        }
    }
}

impl ClassifierEngine {
    pub async fn classify_question(
        &self,
        questions: &[Question],
        recent_context: &str,
        cwd: &str,
        provider: &dyn Provider,
        model: &str,
    ) -> QuestionResult {
        let start = Instant::now();
        let user_prompt =
            question_prompt::user_prompt(questions, self.instructions(), recent_context, cwd);
        let params = ChatParams {
            model: model.to_string(),
            messages: vec![Message::user(&user_prompt)],
            turns: vec![],
            system_prompt: self.question_system_prompt().to_string(),
            tools: vec![],
            max_tokens: 512,
            temperature: Some(0.0),
            thinking: None,
            continuation_intent: None,
            debug_dump_dir: None,
        };

        let stream_res = tokio::time::timeout(self.timeout(), provider.stream_chat(&params)).await;
        let mut stream = match stream_res {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                warn!(error = %e, "classifier-question LLM stream failed");
                self.breaker().record_error(QUESTION_BREAKER_KEY);
                return QuestionResult::error(
                    start.elapsed().as_millis() as u64,
                    format!("stream error: {e}"),
                );
            }
            Err(_) => {
                warn!("classifier-question LLM call timed out");
                self.breaker().record_error(QUESTION_BREAKER_KEY);
                return QuestionResult::error(start.elapsed().as_millis() as u64, "LLM timeout");
            }
        };

        let mut response = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Text { text }) => response.push_str(&text),
                Ok(StreamChunk::Done { .. }) => break,
                Err(e) => {
                    warn!(error = %e, "classifier-question stream chunk error");
                    self.breaker().record_error(QUESTION_BREAKER_KEY);
                    return QuestionResult::error(
                        start.elapsed().as_millis() as u64,
                        format!("chunk error: {e}"),
                    );
                }
                _ => {}
            }
        }

        let result = parse_question_response(&response, start.elapsed().as_millis() as u64);
        if result.error.is_some() {
            self.breaker().record_error(QUESTION_BREAKER_KEY);
        } else {
            self.breaker().record_approval(QUESTION_BREAKER_KEY);
        }
        result
    }
}

fn parse_question_response(raw: &str, duration_ms: u64) -> QuestionResult {
    let json_str = raw
        .trim()
        .strip_prefix("```json")
        .or_else(|| raw.trim().strip_prefix("```"))
        .unwrap_or(raw.trim());
    let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

    let value: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, response = %raw, "classifier-question parse failure");
            return QuestionResult::error(duration_ms, format!("parse failure: {e}"));
        }
    };

    let answers: Vec<Vec<String>> = match value.get("answers").and_then(|v| v.as_array()) {
        Some(arr) => arr
            .iter()
            .map(|inner| {
                inner
                    .as_array()
                    .map(|labels| {
                        labels
                            .iter()
                            .filter_map(|l| l.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect(),
        None => Vec::new(),
    };

    let reason = value
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!(reason = %reason, count = answers.len(), "classifier-question");
    QuestionResult::ok(answers, reason, duration_ms)
}

#[doc(hidden)]
pub fn parse_question_response_for_test(raw: &str) -> QuestionResult {
    parse_question_response(raw, 0)
}
