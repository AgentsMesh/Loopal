use loopal_error::LoopalError;
use loopal_provider_api::MessageOrigin;
use loopal_provider_api::Provider;
use loopal_provider_api::{ContentBlock, Message, MessageRole, project_turns_to_messages};
use loopal_turn::Turn;
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
    /// Number of turns summarized away (== boundary_turn_at).
    pub removed_turn_count: u32,
    /// Number of turns kept after compaction (== input turns.len() - boundary_turn_at).
    pub kept_turn_count: u32,
}

/// Summarize `turns[..boundary_turn_at]` and return a message pair
/// (summary + ack) representing the compacted prefix. Returns `None` when
/// `boundary_turn_at == 0` or out of range (nothing to compact).
pub async fn compact_to_boundary(
    turns: &[Turn],
    provider: &dyn Provider,
    model: &str,
    boundary_turn_at: usize,
    custom_instructions: Option<&str>,
    cancel: &CancellationToken,
) -> Result<Option<CompactOutput>, LoopalError> {
    let Some(old_turns) = slice_old_turns(turns, boundary_turn_at) else {
        return Ok(None);
    };

    let old_messages = project_turns_to_messages(old_turns);
    let touched_files = rank_touched_files(&old_messages, TOUCHED_FILES_HINT_LIMIT);
    let summary_text = produce_summary_text(
        &old_messages,
        provider,
        model,
        custom_instructions,
        &touched_files,
        cancel,
    )
    .await;

    tracing::info!(
        summary_len = summary_text.len(),
        old_turns = old_turns.len(),
        old_messages = old_messages.len(),
        touched_files = touched_files.len(),
        "compaction summary produced"
    );

    let removed_turn_count = boundary_turn_at as u32;
    let kept_turn_count = turns.len().saturating_sub(boundary_turn_at) as u32;
    Ok(Some(build_compact_output(
        summary_text,
        old_messages.len(),
        touched_files,
        removed_turn_count,
        kept_turn_count,
    )))
}

fn slice_old_turns(turns: &[Turn], boundary_turn_at: usize) -> Option<&[Turn]> {
    if boundary_turn_at == 0 || boundary_turn_at > turns.len() {
        return None;
    }
    Some(&turns[..boundary_turn_at])
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
    removed_turn_count: u32,
    kept_turn_count: u32,
) -> CompactOutput {
    let summary_msg = build_summary_message(&summary_text, old_count, &touched_files);
    let ack_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Text {
            text: "Understood. I'll continue from this working state.".to_string(),
        }],
        origin: Some(MessageOrigin::CompactionSummary),
        ephemeral_in_history: false,
    };
    CompactOutput {
        summary_msg,
        ack_msg,
        touched_files,
        old_count,
        removed_turn_count,
        kept_turn_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_turn::TurnTrigger;

    fn user_turn(text: &str) -> Turn {
        Turn::new(TurnTrigger::UserInput {
            envelope_id: text.into(),
            content: text.into(),
            images: Vec::new(),
        })
    }

    #[test]
    fn slice_returns_none_at_zero_boundary() {
        let t = vec![user_turn("a"), user_turn("b")];
        assert!(slice_old_turns(&t, 0).is_none());
    }

    #[test]
    fn slice_returns_none_past_end() {
        let t = vec![user_turn("a")];
        assert!(slice_old_turns(&t, 5).is_none());
    }

    #[test]
    fn slice_takes_prefix() {
        let t = vec![user_turn("a"), user_turn("b"), user_turn("c")];
        assert_eq!(slice_old_turns(&t, 2).unwrap().len(), 2);
    }

    #[test]
    fn build_compact_output_returns_user_and_assistant() {
        let out = build_compact_output("body".into(), 3, vec![], 2, 1);
        assert_eq!(out.summary_msg.role, MessageRole::User);
        assert_eq!(out.ack_msg.role, MessageRole::Assistant);
        assert_eq!(out.old_count, 3);
        assert_eq!(out.removed_turn_count, 2);
        assert_eq!(out.kept_turn_count, 1);
    }
}
