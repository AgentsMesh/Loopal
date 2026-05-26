use loopal_turn::{CompactionRehydrate, CompactionSummary};

use super::super::message::{ContentBlock, Message, MessageRole};
use super::super::origin::MessageOrigin;

pub(super) fn project_compaction_summary(s: &CompactionSummary) -> Vec<Message> {
    vec![
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: s.summary_text.clone(),
            }],
            origin: Some(MessageOrigin::CompactionSummary),
            ephemeral_in_history: false,
        },
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text {
                text: s.ack_text.clone(),
            }],
            origin: Some(MessageOrigin::CompactionSummary),
            ephemeral_in_history: false,
        },
    ]
}

pub(super) fn project_compaction_rehydrate(r: &CompactionRehydrate) -> Vec<Message> {
    if r.files.is_empty() {
        return Vec::new();
    }
    let assistant_blocks: Vec<ContentBlock> = r
        .files
        .iter()
        .map(|f| ContentBlock::ToolUse {
            id: f.tool_call_id.as_str().to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({ "file_path": f.path }),
        })
        .collect();
    let mut user_blocks: Vec<ContentBlock> = r
        .files
        .iter()
        .map(|f| ContentBlock::ToolResult {
            tool_use_id: f.tool_call_id.as_str().to_string(),
            content: f.content.clone(),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        })
        .collect();
    if let Some(note) = r.partial_note.as_ref() {
        user_blocks.push(ContentBlock::Text { text: note.clone() });
    }
    vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: assistant_blocks,
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: user_blocks,
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        },
    ]
}
