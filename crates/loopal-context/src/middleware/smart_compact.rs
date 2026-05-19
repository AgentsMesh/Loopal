//! Build a compaction summary from a conversation segment.
//!
//! The caller selects a `boundary_at` cut point: every message with
//! index `< boundary_at` is summarized and discarded; everything at or
//! after is preserved verbatim. The returned `(summary_msg, ack_msg)`
//! pair is persisted by the runtime (so `Marker::CompactBoundary` can
//! later anchor on the summary's message id) and pushed at the head of
//! the new conversation.

use loopal_error::LoopalError;
use loopal_message::{ContentBlock, Message, MessageOrigin, MessageRole};
use loopal_provider_api::Provider;
use tokio_util::sync::CancellationToken;

use super::bare_summary::{bare_summary, build_summary_message};
use super::conversation_text::build_conversation_text;
use super::smart_compact_llm::call_summarization_llm;
use super::summary_parse::extract_summary;
use super::touched_files::{TouchedFile, rank_touched_files};
use crate::compact_config::TOUCHED_FILES_HINT_LIMIT;

#[derive(Debug)]
pub struct CompactOutput {
    pub summary_msg: Message,
    pub ack_msg: Message,
    pub touched_files: Vec<TouchedFile>,
    pub old_count: usize,
}

/// Top-level orchestrator. Three small steps, each independently testable:
///   1. `slice_old_messages` — domain rule: what gets summarized.
///   2. `produce_summary_text` — pure-ish dispatch: LLM or `bare_summary`.
///   3. `build_compact_output` — sync constructor of the message pair.
pub async fn compact_to_boundary(
    messages: &[Message],
    provider: &dyn Provider,
    model: &str,
    boundary_at: usize,
    custom_instructions: Option<&str>,
    cancel: &CancellationToken,
) -> Result<Option<CompactOutput>, LoopalError> {
    let Some(old_messages) = slice_old_messages(messages, boundary_at) else {
        return Ok(None);
    };

    let touched_files = rank_touched_files(old_messages, TOUCHED_FILES_HINT_LIMIT);
    let summary_text = produce_summary_text(
        old_messages,
        provider,
        model,
        custom_instructions,
        &touched_files,
        cancel,
    )
    .await;

    tracing::info!(
        summary_len = summary_text.len(),
        old_messages = old_messages.len(),
        touched_files = touched_files.len(),
        "compaction summary produced"
    );

    Ok(Some(build_compact_output(
        summary_text,
        old_messages.len(),
        touched_files,
    )))
}

fn slice_old_messages(messages: &[Message], boundary_at: usize) -> Option<&[Message]> {
    if boundary_at == 0 || boundary_at > messages.len() {
        return None;
    }
    Some(&messages[..boundary_at])
}

/// Call the LLM with retry; on any terminal failure fall back to a
/// deterministic outline so compaction always produces *something*.
/// Pulling this out lets tests exercise the fallback path without a
/// full provider stub.
async fn produce_summary_text(
    old_messages: &[Message],
    provider: &dyn Provider,
    model: &str,
    custom_instructions: Option<&str>,
    touched_files: &[TouchedFile],
    cancel: &CancellationToken,
) -> String {
    let conversation_text = build_conversation_text(old_messages);
    match call_summarization_llm(
        provider,
        model,
        &conversation_text,
        custom_instructions,
        cancel,
    )
    .await
    {
        Ok(raw) => {
            let extracted = extract_summary(&raw).to_string();
            if extracted.is_empty() {
                tracing::warn!(
                    fallback = "bare_summary",
                    cause = "empty_llm_response",
                    old_messages = old_messages.len(),
                    touched_files = touched_files.len(),
                    "compaction LLM returned no summary; using deterministic fallback"
                );
                bare_summary(old_messages, touched_files)
            } else {
                extracted
            }
        }
        Err(e) => {
            tracing::warn!(
                fallback = "bare_summary",
                cause = "llm_call_exhausted",
                error = %e,
                old_messages = old_messages.len(),
                touched_files = touched_files.len(),
                "compaction LLM exhausted retries; using deterministic fallback"
            );
            bare_summary(old_messages, touched_files)
        }
    }
}

fn build_compact_output(
    summary_text: String,
    old_count: usize,
    touched_files: Vec<TouchedFile>,
) -> CompactOutput {
    let summary_msg = build_summary_message(&summary_text, old_count, &touched_files);
    let ack_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Text {
            text: "Understood. I'll continue from this working state.".to_string(),
        }],
        origin: Some(MessageOrigin::CompactionSummary),
    };
    CompactOutput {
        summary_msg,
        ack_msg,
        touched_files,
        old_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_returns_none_at_zero_boundary() {
        let m = vec![Message::user("a"), Message::user("b")];
        assert!(slice_old_messages(&m, 0).is_none());
    }

    #[test]
    fn slice_returns_none_past_end() {
        let m = vec![Message::user("a")];
        assert!(slice_old_messages(&m, 5).is_none());
    }

    #[test]
    fn slice_takes_prefix() {
        let m = vec![Message::user("a"), Message::user("b"), Message::user("c")];
        assert_eq!(slice_old_messages(&m, 2).unwrap().len(), 2);
    }

    #[test]
    fn build_compact_output_returns_user_and_assistant() {
        let out = build_compact_output("body".into(), 3, vec![]);
        assert_eq!(out.summary_msg.role, MessageRole::User);
        assert_eq!(out.ack_msg.role, MessageRole::Assistant);
        assert_eq!(out.old_count, 3);
    }
}
