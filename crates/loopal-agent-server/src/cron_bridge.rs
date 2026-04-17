//! Cron job bridge — polls the root agent's `CronScheduler` at a fixed
//! interval and emits `CronsChanged` whenever the *job set* changes.
//!
//! Diff-skip is based on the unordered set of `(id, prompt, recurring)` —
//! **not** `next_fire`. Two reasons:
//! 1. `CronScheduler::list()` recomputes `next_fire` relative to the
//!    current time on every call; including it in the diff would defeat
//!    the optimization (every tick would look "changed").
//! 2. `CronScheduler` internally calls `Vec::remove(index)` when one-shot
//!    jobs fire, which shifts later entries left. An order-sensitive diff
//!    would spuriously emit on every such fire even though the surviving
//!    job set is unchanged.
//!
//! The countdown is recomputed by the TUI each frame via
//! `cron_duration_format::format_next_fire_ms(now)`, so the TUI stays
//! fresh without the bridge firing redundant events.
//!
//! Scope: only the root agent's scheduler is observed. Sub-agents maintain
//! their own schedulers via `AgentShared`, but they are deliberately not
//! exposed — consistent with how `bg_task_bridge` only reports root-level
//! background tasks. Extending to sub-agents is YAGNI until requested.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use loopal_protocol::{AgentEventPayload, CronJobSnapshot};
use loopal_runtime::frontend::traits::AgentFrontend;
use loopal_scheduler::{CronJobInfo, CronScheduler};

/// Default poll interval in production. Matches `bg_task_bridge`'s
/// `OUTPUT_SAMPLE_INTERVAL` — both are "low-pressure observation" taps.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Stable identity for diff-skip. The `id` is kept verbatim (8 chars);
/// the rest of the identifying payload (prompt, recurring, cron_expr) is
/// reduced to a 64-bit hash to avoid cloning large prompt strings on
/// every poll.
///
/// **`content_hash` is not stable across Rust versions.** It is produced
/// by [`std::collections::hash_map::DefaultHasher`] which may change its
/// algorithm between releases. This is safe for the current use case
/// because `CronIdentity` is only compared in-process during the lifetime
/// of a single bridge. Do **not** persist `content_hash` to disk, embed
/// it in IPC messages, or compare values produced by different processes.
///
/// If new identifying fields are added to [`CronJobSnapshot`], remember
/// to extend [`From<&CronJobSnapshot>`] below or the diff-skip will
/// ignore changes in that field.
#[derive(Debug, PartialEq, Eq, Hash)]
struct CronIdentity {
    id: String,
    content_hash: u64,
}

impl From<&CronJobSnapshot> for CronIdentity {
    fn from(s: &CronJobSnapshot) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        s.prompt.hash(&mut hasher);
        s.recurring.hash(&mut hasher);
        s.cron_expr.hash(&mut hasher);
        s.durable.hash(&mut hasher);
        Self {
            id: s.id.clone(),
            content_hash: hasher.finish(),
        }
    }
}

/// Spawn a bridge polling at the default 2-second cadence.
pub fn spawn(scheduler: Arc<CronScheduler>, frontend: Arc<dyn AgentFrontend>) -> JoinHandle<()> {
    spawn_with_interval(scheduler, frontend, DEFAULT_POLL_INTERVAL)
}

/// Spawn a bridge with a custom poll interval (exposed for tests).
pub fn spawn_with_interval(
    scheduler: Arc<CronScheduler>,
    frontend: Arc<dyn AgentFrontend>,
    poll_interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Skip missed ticks rather than bursting — cron polling is a sample,
        // not an accumulator; replaying old ticks would just duplicate work.
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let initial = snapshot_all(&scheduler).await;
        if let Err(e) = frontend
            .emit(AgentEventPayload::CronsChanged {
                crons: initial.clone(),
            })
            .await
        {
            tracing::warn!(error = %e, "failed to emit initial CronsChanged");
        }
        let mut previous_ids = to_identity_set(&initial);

        loop {
            interval.tick().await;
            let current = snapshot_all(&scheduler).await;
            let current_ids = to_identity_set(&current);
            if current_ids == previous_ids {
                continue;
            }
            if let Err(e) = frontend
                .emit(AgentEventPayload::CronsChanged {
                    crons: current.clone(),
                })
                .await
            {
                tracing::warn!(error = %e, "failed to emit CronsChanged");
            }
            previous_ids = current_ids;
        }
    })
}

fn to_identity_set(snapshots: &[CronJobSnapshot]) -> HashSet<CronIdentity> {
    snapshots.iter().map(CronIdentity::from).collect()
}

async fn snapshot_all(scheduler: &CronScheduler) -> Vec<CronJobSnapshot> {
    scheduler
        .list()
        .await
        .into_iter()
        .map(to_snapshot)
        .collect()
}

fn to_snapshot(info: CronJobInfo) -> CronJobSnapshot {
    CronJobSnapshot {
        id: info.id,
        cron_expr: info.cron_expr,
        prompt: info.prompt.replace('\n', " ").replace('\r', ""),
        recurring: info.recurring,
        created_at_unix_ms: info.created_at.timestamp_millis(),
        next_fire_unix_ms: info.next_fire.map(|t| t.timestamp_millis()),
        durable: info.durable,
    }
}

// Expose private helpers to the sibling `cron_bridge_tests` module so that
// branches like `to_snapshot` with `next_fire: None` can be unit-tested
// directly (scheduler.list() never yields None for valid expressions).
#[cfg(test)]
pub(super) use to_snapshot as to_snapshot_for_test;

#[cfg(test)]
#[path = "cron_bridge_tests.rs"]
mod tests;
