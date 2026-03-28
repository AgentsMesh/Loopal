/// Reconnection policy for HTTP-based MCP servers.
///
/// Stdio transports do not reconnect (subprocess lifecycle is one-shot).
/// HTTP transports use exponential backoff with configurable max attempts.
use std::time::Duration;

use loopal_error::McpError;
use tracing::{info, warn};

use crate::connection::McpConnection;
use crate::types::ConnectionStatus;
use loopal_config::McpServerConfig;

/// Exponential backoff policy.
pub struct ReconnectPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub backoff_factor: f64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 6,
            base_delay: Duration::from_secs(2),
            backoff_factor: 2.0,
        }
    }
}

impl ReconnectPolicy {
    /// Compute delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        self.base_delay
            .mul_f64(self.backoff_factor.powi(attempt as i32))
    }

    /// Whether the config is eligible for reconnection (HTTP only).
    pub fn is_reconnectable(config: &McpServerConfig) -> bool {
        matches!(config, McpServerConfig::StreamableHttp { .. })
    }
}

/// Attempt to reconnect an HTTP connection with exponential backoff.
///
/// Returns `Ok(())` if reconnection succeeds, `Err` if all attempts exhausted.
pub async fn reconnect_loop(
    conn: &mut McpConnection,
    policy: &ReconnectPolicy,
) -> Result<(), McpError> {
    if !ReconnectPolicy::is_reconnectable(&conn.config) {
        return Err(McpError::ConnectionFailed(
            "stdio connections do not support reconnection".into(),
        ));
    }

    for attempt in 0..policy.max_attempts {
        let delay = policy.delay_for_attempt(attempt);
        info!(
            server = %conn.name,
            attempt = attempt + 1,
            max = policy.max_attempts,
            delay_ms = delay.as_millis() as u64,
            "reconnecting MCP server"
        );

        tokio::time::sleep(delay).await;

        conn.connect().await;
        if conn.status.is_connected() {
            info!(server = %conn.name, "reconnection succeeded");
            return Ok(());
        }

        warn!(
            server = %conn.name,
            attempt = attempt + 1,
            "reconnection attempt failed"
        );
    }

    let msg = format!(
        "reconnection exhausted after {} attempts",
        policy.max_attempts
    );
    conn.status = ConnectionStatus::Failed(msg.clone());
    Err(McpError::ConnectionFailed(msg))
}
