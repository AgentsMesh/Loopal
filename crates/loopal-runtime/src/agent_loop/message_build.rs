use loopal_message::{ContentBlock, ImageSource, Message, MessageRole};
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
    }
}
