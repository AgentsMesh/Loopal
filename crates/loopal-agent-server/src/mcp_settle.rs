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
    tokio::spawn(proxy_mcp_settle_poll(kernel, poll, quiet_streak_to_settle));
}

async fn proxy_mcp_settle_poll(
    kernel: std::sync::Weak<Kernel>,
    poll: std::time::Duration,
    quiet_streak_to_settle: u32,
) {
    let mut interval = tokio::time::interval(poll);
    interval.tick().await;
    let mut quiet_streak: u32 = 0;
    loop {
        interval.tick().await;
        let Some(k) = kernel.upgrade() else {
            return;
        };
        let added = k.register_all_settled_mcp_tools().await;
        quiet_streak = update_quiet_streak(quiet_streak, added);
        if quiet_streak >= quiet_streak_to_settle {
            tracing::debug!(
                quiet_streak,
                "proxy MCP settle-poll: tool surface stable, terminating"
            );
            return;
        }
    }
}

fn update_quiet_streak(current: u32, added: usize) -> u32 {
    if added == 0 {
        current.saturating_add(1)
    } else {
        0
    }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{proxy_mcp_settle_poll, update_quiet_streak};

    #[test]
    fn quiet_streak_resets_when_new_tools_arrive() {
        assert_eq!(update_quiet_streak(2, 0), 3);
        assert_eq!(update_quiet_streak(2, 1), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn settle_poll_exits_when_kernel_is_dropped() {
        let task = tokio::spawn(proxy_mcp_settle_poll(
            std::sync::Weak::new(),
            std::time::Duration::from_secs(1),
            1,
        ));
        tokio::task::yield_now().await;
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn settle_poll_terminates_after_a_quiet_tool_surface() {
        let kernel =
            Arc::new(loopal_kernel::Kernel::new(loopal_config::Settings::default()).unwrap());
        let task = tokio::spawn(proxy_mcp_settle_poll(
            Arc::downgrade(&kernel),
            std::time::Duration::from_secs(1),
            2,
        ));
        tokio::task::yield_now().await;
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        task.await.unwrap();
    }
}
