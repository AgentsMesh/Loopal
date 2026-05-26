use loopal_kernel::Kernel;

// Agent uses McpProxyClient and cannot subscribe to Hub-side LocalMcpProvider
// settle events. Poll the Hub at low cadence while tools arrive; back off
// once the tool surface is stable so we don't tick forever for the kernel's
// lifetime. register_all_settled_mcp_tools returns newly-registered count,
// so a streak of zeros means "settled".
pub(super) fn spawn_proxy_mcp_settle_poll(kernel: std::sync::Weak<Kernel>) {
    let poll = mcp_settle_poll_interval();
    let quiet_streak_to_settle: u32 = std::env::var("LOOPAL_MCP_POLL_QUIET_STREAK")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(4);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll);
        interval.tick().await;
        let mut quiet_streak: u32 = 0;
        loop {
            interval.tick().await;
            let Some(k) = kernel.upgrade() else {
                return;
            };
            let added = k.register_all_settled_mcp_tools().await;
            if added == 0 {
                quiet_streak = quiet_streak.saturating_add(1);
                if quiet_streak >= quiet_streak_to_settle {
                    tracing::debug!(
                        quiet_streak,
                        "proxy MCP settle-poll: tool surface stable, terminating"
                    );
                    return;
                }
            } else {
                quiet_streak = 0;
            }
        }
    });
}

pub(super) fn mcp_startup_wait() -> std::time::Duration {
    let secs = std::env::var("LOOPAL_MCP_STARTUP_WAIT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);
    std::time::Duration::from_secs(secs)
}

fn mcp_settle_poll_interval() -> std::time::Duration {
    let secs = std::env::var("LOOPAL_MCP_POLL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(3);
    std::time::Duration::from_secs(secs)
}
