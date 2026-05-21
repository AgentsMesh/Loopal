use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::AgentEventPayload;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn spawn_hub_health_poller(&self) {
        let Some(client) = self.params.deps.kernel.secret_client() else {
            return;
        };
        let Some(health) = client.health() else {
            return;
        };
        let tick = hub_health_tick_interval();
        let weak_frontend = Arc::downgrade(&self.params.deps.frontend);
        tokio::spawn(async move {
            // Synthetic startup emit: if Hub is ALREADY degraded when the
            // watcher boots (Agent restart, slow Hub bootstrap), users would
            // otherwise see no indication until recovery. Read the snapshot
            // before the tick loop so the first event surfaces immediately.
            let mut last_degraded = health.is_degraded();
            let mut last_degraded_at: Option<u64> = if last_degraded {
                let since = health.degraded_at_unix_ms().unwrap_or_else(now_unix_ms);
                let Some(f) = weak_frontend.upgrade() else {
                    return;
                };
                if let Err(e) = f
                    .emit(AgentEventPayload::HubDegraded {
                        since_unix_ms: since,
                    })
                    .await
                {
                    tracing::warn!(error = %e, "hub_health initial emit failed, stopping watcher");
                    return;
                }
                Some(since)
            } else {
                None
            };
            let mut interval = tokio::time::interval(tick);
            interval.tick().await;
            loop {
                interval.tick().await;
                let now_degraded = health.is_degraded();
                if now_degraded == last_degraded {
                    continue;
                }
                let Some(f) = weak_frontend.upgrade() else {
                    break;
                };
                let payload = if now_degraded {
                    let since = health.degraded_at_unix_ms().unwrap_or_else(now_unix_ms);
                    last_degraded_at = Some(since);
                    AgentEventPayload::HubDegraded {
                        since_unix_ms: since,
                    }
                } else {
                    let dur = last_degraded_at
                        .map(|s| now_unix_ms().saturating_sub(s))
                        .unwrap_or(0);
                    last_degraded_at = None;
                    AgentEventPayload::HubRecovered { duration_ms: dur }
                };
                if let Err(e) = f.emit(payload).await {
                    tracing::warn!(error = %e, "hub_health emit failed, stopping watcher");
                    break;
                }
                last_degraded = now_degraded;
            }
        });
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hub_health_tick_interval() -> Duration {
    let secs = std::env::var("LOOPAL_HUB_HEALTH_TICK_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(1);
    Duration::from_secs(secs)
}
