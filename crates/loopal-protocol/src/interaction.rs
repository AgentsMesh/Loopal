use std::time::Duration;

/// Maximum time the Hub owns a pending human interaction.
pub const DEFAULT_INTERACTION_LIFETIME: Duration = Duration::from_secs(600);

/// Time reserved for the Hub to enqueue the terminal event and reply upstream.
pub const INTERACTION_RPC_COMPLETION_GRACE: Duration = Duration::from_secs(5);

/// Agent-side bound encompassing the Hub lifetime and its completion work.
pub const DEFAULT_INTERACTION_RPC_TIMEOUT: Duration = Duration::from_secs(
    DEFAULT_INTERACTION_LIFETIME.as_secs() + INTERACTION_RPC_COMPLETION_GRACE.as_secs(),
);
