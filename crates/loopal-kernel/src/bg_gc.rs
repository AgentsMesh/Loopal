use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::BgTaskConfig;
use loopal_tool_background::BackgroundTaskStore;

pub fn spawn_bg_gc_tick(store: Arc<BackgroundTaskStore>, config: BgTaskConfig) {
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    let interval = config.gc_interval();
    let retention = config.terminal_retention();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval.max(Duration::from_millis(1)));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let evicted = store.evict_terminal(retention);
            if evicted > 0 {
                tracing::info!(evicted, "evicted terminal bg tasks");
            }
        }
    });
}
