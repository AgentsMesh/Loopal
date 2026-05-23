use loopal_message::{ContentBlock, ImageSource, Message, MessageOrigin, MessageRole};
use loopal_protocol::{Envelope, MessageSource};

/// Build a user Message from an Envelope, converting UserContent into ContentBlocks.
pub fn build_user_message(env: &Envelope) -> Message {
    let text = match &env.source {
        MessageSource::Human => env.content.text.clone(),
        MessageSource::Agent(name) => format!("[from: {}] {}", name, env.content.text),
        MessageSource::Channel { channel, from } => {
            format!("[from: #{}/{}] {}", channel, from, env.content.text)
        }
        MessageSource::Scheduled => format!("[scheduled] {}", env.content.text),
        MessageSource::System(_) => env.content.text.clone(),
    };
    let mut blocks: Vec<ContentBlock> = Vec::new();
    if !text.is_empty() {
        blocks.push(ContentBlock::Text { text });
    }
    for img in &env.content.images {
        blocks.push(ContentBlock::Image {
            source: ImageSource {
                source_type: "base64".to_string(),
                media_type: img.media_type.clone(),
                data: img.data.clone(),
            },
        });
    }
    Message {
        id: None,
        role: MessageRole::User,
        content: blocks,
        origin: Some(message_origin_for(&env.source)),
        ephemeral_in_history: false,
    }
}

// reason: protocol → message audit projection. Lives here (not in protocol or
// message crate) so neither cross-depends on the other; runtime is the
// natural owner since it consumes both shapes.
pub fn message_origin_for(src: &MessageSource) -> MessageOrigin {
    match src {
        MessageSource::Human => MessageOrigin::Human,
        MessageSource::Scheduled => MessageOrigin::Scheduled,
        MessageSource::Agent(addr) => MessageOrigin::Agent {
            label: addr.to_string(),
        },
        MessageSource::Channel { channel, from } => MessageOrigin::Channel {
            name: channel.clone(),
            from: from.to_string(),
        },
        MessageSource::System(kind) => match kind.as_str() {
            "goal_continuation" => MessageOrigin::GoalContinuation,
            "governance_compensation" => MessageOrigin::GovernanceCompensation,
            "governance_feedback" => MessageOrigin::GovernanceFeedback,
            "stop_feedback" => MessageOrigin::StopFeedback,
            "config_refresh" => MessageOrigin::ConfigRefresh,
            "compaction_summary" => MessageOrigin::CompactionSummary,
            "compaction_rehydrate" => MessageOrigin::CompactionRehydrate,
            other => MessageOrigin::Other {
                label: other.to_string(),
            },
        },
    }
}
