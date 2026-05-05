use std::borrow::Cow;

use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider_api::ChatParams;

use super::AnthropicProvider;
use crate::model_info::get_model_info;

pub(super) const CONTINUATION_MARKER: &str = "[Continue from where you left off]";

impl AnthropicProvider {
    pub(super) fn finalize_messages_impl<'a>(&self, params: &'a ChatParams) -> Cow<'a, [Message]> {
        let supports_prefill = get_model_info(&params.model)
            .map(|m| m.supports_prefill)
            .unwrap_or(true);
        let needs_user_tail = !supports_prefill || params.continuation_intent.is_some();
        if !needs_user_tail {
            return Cow::Borrowed(&params.messages);
        }
        if matches!(params.messages.last(), Some(m) if m.role == MessageRole::User) {
            return Cow::Borrowed(&params.messages);
        }
        let mut owned = params.messages.to_vec();
        owned.push(Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: CONTINUATION_MARKER.to_string(),
            }],
        });
        Cow::Owned(owned)
    }
}
