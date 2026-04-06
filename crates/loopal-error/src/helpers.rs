use crate::{LoopalError, ProviderError};

impl ProviderError {
    /// Check if this is a rate limit error
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, ProviderError::RateLimited { .. })
    }

    /// Check if this error is retryable (rate limit, server errors, network errors, etc.)
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::RateLimited { .. } => true,
            // Network-level errors (connection reset, timeout, DNS) are transient.
            ProviderError::Http(_) => true,
            ProviderError::Api { status, message } => {
                // 400 with context overflow keywords is deterministic — never retryable.
                // Generic "invalid_request_error" is excluded: it covers many
                // non-overflow 400s (prefill rejection, malformed blocks, etc.).
                if *status == 400
                    && (message.contains("prompt is too long")
                        || message.contains("maximum context length"))
                {
                    return false;
                }
                matches!(status, 429 | 500 | 502 | 503 | 529)
            }
            ProviderError::ContextOverflow { .. } => false,
            _ => false,
        }
    }

    /// Check if this error indicates the prompt exceeded the model's context window.
    pub fn is_context_overflow(&self) -> bool {
        match self {
            ProviderError::ContextOverflow { .. } => true,
            ProviderError::Api { status, message } if *status == 400 => {
                message.contains("prompt is too long") || message.contains("maximum context length")
            }
            _ => false,
        }
    }

    /// Get the retry-after duration in milliseconds, if this is a rate limit error
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            _ => None,
        }
    }
}

impl LoopalError {
    /// Check if this is a rate limit error
    pub fn is_rate_limited(&self) -> bool {
        matches!(
            self,
            LoopalError::Provider(ProviderError::RateLimited { .. })
        )
    }

    /// Check if this error is retryable (rate limit, server errors, etc.)
    pub fn is_retryable(&self) -> bool {
        matches!(self, LoopalError::Provider(e) if e.is_retryable())
    }

    /// Get the retry-after duration in milliseconds, if this is a rate limit error
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            LoopalError::Provider(ProviderError::RateLimited { retry_after_ms }) => {
                Some(*retry_after_ms)
            }
            _ => None,
        }
    }

    /// Check if this error indicates the prompt exceeded the model's context window.
    pub fn is_context_overflow(&self) -> bool {
        matches!(self, LoopalError::Provider(e) if e.is_context_overflow())
    }
}
