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
            ProviderError::Api { status, .. } => matches!(status, 429 | 500 | 502 | 503 | 529),
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
