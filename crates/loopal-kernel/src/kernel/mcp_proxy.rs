use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::Kernel;

impl Kernel {
    pub(super) async fn wait_for_proxy_mcp(&self, max_wait: Duration) -> bool {
        let expected = self
            .settings
            .mcp_servers
            .iter()
            .filter(|(_, config)| config.enabled())
            .map(|(name, _)| name.as_str())
            .collect::<HashSet<_>>();
        if expected.is_empty() {
            return true;
        }
        let deadline = Instant::now() + max_wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let snapshots = match tokio::time::timeout(
                remaining,
                self.mcp.provider().snapshot(loopal_mcp::HUB_RPC_BUDGET),
            )
            .await
            {
                Ok(value) => value,
                Err(_) => return false,
            };
            if expected.iter().all(|name| {
                snapshots.iter().any(|server| {
                    server.name == *name
                        && (server.status == "connected" || server.status.starts_with("failed:"))
                })
            }) {
                return true;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            tokio::time::sleep(remaining.min(Duration::from_millis(25))).await;
        }
    }
}
