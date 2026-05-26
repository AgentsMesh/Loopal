use loopal_turn::TurnTrigger;

use super::super::message::{ContentBlock, ImageSource, Message, MessageRole};
use super::super::origin::MessageOrigin;

pub(super) fn project_trigger(trigger: &TurnTrigger) -> Option<Message> {
    match trigger {
        TurnTrigger::UserInput {
            content, images, ..
        } => Some(text_user_with_images(
            content,
            images,
            Some(MessageOrigin::Human),
        )),
        TurnTrigger::Cron { content, .. } => Some(text_user(
            &format!("[scheduled] {content}"),
            Some(MessageOrigin::Scheduled),
        )),
        TurnTrigger::Agent { from, content, .. } => Some(text_user(
            &format!("[from: {from}] {content}"),
            Some(MessageOrigin::Agent {
                label: from.clone(),
            }),
        )),
        TurnTrigger::Channel {
            channel,
            from,
            content,
            ..
        } => Some(text_user(
            &format!("[from: #{channel}/{from}] {content}"),
            Some(MessageOrigin::Channel {
                name: channel.clone(),
                from: from.clone(),
            }),
        )),
        TurnTrigger::GoalContinuation { content, .. } => {
            Some(text_user(content, Some(MessageOrigin::GoalContinuation)))
        }
        TurnTrigger::BackgroundHook {
            hook_kind, content, ..
        } => Some(text_user(
            content,
            Some(MessageOrigin::Other {
                label: hook_kind.clone(),
            }),
        )),
        TurnTrigger::Resume => None,
    }
}

pub(super) fn text_user(text: &str, origin: Option<MessageOrigin>) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
        origin,
        ephemeral_in_history: false,
    }
}

fn text_user_with_images(
    text: &str,
    images: &[loopal_tool_invocation::ToolImageBlock],
    origin: Option<MessageOrigin>,
) -> Message {
    let mut content: Vec<ContentBlock> = Vec::new();
    if !text.is_empty() {
        content.push(ContentBlock::Text {
            text: text.to_string(),
        });
    }
    for img in images {
        match img {
            loopal_tool_invocation::ToolImageBlock::Inline { media_type, data } => {
                content.push(ContentBlock::Image {
                    source: ImageSource {
                        source_type: "base64".to_string(),
                        media_type: media_type.clone(),
                        data: data.clone(),
                    },
                });
            }
            // SessionResource resolved at provider send time, not projection.
            loopal_tool_invocation::ToolImageBlock::SessionResource { .. } => {}
        }
    }
    Message {
        id: None,
        role: MessageRole::User,
        content,
        origin,
        ephemeral_in_history: false,
    }
}
