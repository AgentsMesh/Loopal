use crate::{LoopalError, ProviderError};

/// Upper bound for a provider-directed retry delay. This is enforced at the
/// error contract so every caller, including main turns and compaction, makes
/// progress even if an adapter or test double supplies an extreme value.
pub const MAX_RETRY_AFTER_MS: u64 = 5 * 60 * 1000;

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
            ProviderError::Api { status, .. } => {
                matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
            }
            // A stream that closes before its protocol terminal marker is a
            // transient transport failure when no semantic output was seen.
            ProviderError::StreamEnded => true,
            ProviderError::ContextOverflow { .. } => false,
            _ => false,
        }
    }

    /// Whether the error is a context-window overflow.
    ///
    /// Only matches the explicit `ContextOverflow` variant. Each provider's
    /// `Provider::classify_error` is responsible for translating its own 400-body
    /// keywords into this classification — keeping protocol-specific text out
    /// of the generic error layer.
    pub fn is_context_overflow(&self) -> bool {
        matches!(self, ProviderError::ContextOverflow { .. })
    }

    /// Get the bounded server-directed retry delay in milliseconds.
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            ProviderError::Api { retry_after_ms, .. } => *retry_after_ms,
            _ => None,
        }
        .map(|delay| delay.min(MAX_RETRY_AFTER_MS))
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

    /// Get the bounded server-directed retry delay in milliseconds.
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            LoopalError::Provider(error) => error.retry_after_ms(),
            _ => None,
        }
    }

    /// Check if this error indicates the prompt exceeded the model's context window.
    pub fn is_context_overflow(&self) -> bool {
        matches!(self, LoopalError::Provider(e) if e.is_context_overflow())
    }
}
