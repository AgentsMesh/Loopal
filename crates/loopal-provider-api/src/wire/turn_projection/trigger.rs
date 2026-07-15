use loopal_turn::TurnTrigger;

use super::super::message::{ContentBlock, ImageSource, Message, MessageRole};
use super::super::origin::MessageOrigin;
use super::prefix::trigger_llm_text;

pub(super) fn project_trigger(trigger: &TurnTrigger) -> Option<Message> {
    // UserInput carries images — project structurally, not via text prefix.
    if let TurnTrigger::UserInput {
        content, images, ..
    }
    | TurnTrigger::SkillInput {
        content, images, ..
    } = trigger
    {
        return Some(text_user_with_images(
            content,
            images,
            trigger_origin(trigger),
        ));
    }
    let text = trigger_llm_text(trigger)?;
    Some(text_user(&text, trigger_origin(trigger)))
}

fn trigger_origin(trigger: &TurnTrigger) -> Option<MessageOrigin> {
    match trigger {
        TurnTrigger::UserInput { .. } => Some(MessageOrigin::Human),
        TurnTrigger::SkillInput {
            name, user_args, ..
        } => Some(MessageOrigin::HumanSkill {
            name: name.clone(),
            user_args: user_args.clone(),
        }),
        TurnTrigger::Cron { .. } => Some(MessageOrigin::Scheduled),
        TurnTrigger::Agent { from, .. } | TurnTrigger::AgentResult { from, .. } => {
            Some(MessageOrigin::Agent {
                label: from.clone(),
            })
        }
        TurnTrigger::Channel { channel, from, .. } => Some(MessageOrigin::Channel {
            name: channel.clone(),
            from: from.clone(),
        }),
        TurnTrigger::GoalContinuation { .. } => Some(MessageOrigin::GoalContinuation),
        TurnTrigger::BackgroundHook { hook_kind, .. } => Some(MessageOrigin::Other {
            label: hook_kind.clone(),
        }),
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
