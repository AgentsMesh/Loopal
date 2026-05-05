use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ErrorClass, default_classify_error};

use super::AnthropicProvider;

impl AnthropicProvider {
    pub(super) fn classify_error_impl(&self, err: &LoopalError) -> ErrorClass {
        if let LoopalError::Provider(ProviderError::Api {
            status: 400,
            message,
        }) = err
        {
            if message.contains("does not support assistant message prefill") {
                return ErrorClass::PrefillRejected;
            }
            if is_anthropic_server_block_keyword(message) {
                return ErrorClass::ServerBlockError;
            }
            if is_anthropic_context_overflow_keyword(message) {
                return ErrorClass::ContextOverflow;
            }
        }
        default_classify_error(err)
    }
}

fn is_anthropic_server_block_keyword(message: &str) -> bool {
    message.contains("code_execution")
        && (message.contains("without a corresponding") || message.contains("tool_result"))
}

fn is_anthropic_context_overflow_keyword(message: &str) -> bool {
    message.contains("prompt is too long")
        || message.contains("maximum context length")
        || message.contains("exceed context limit")
}
