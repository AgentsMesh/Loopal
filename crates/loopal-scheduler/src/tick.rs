//! Background tick loop — runs every second, checks tasks, sends triggers.

use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::clock::Clock;
use crate::task::ScheduledTask;
use crate::trigger::ScheduledTrigger;

/// Core tick loop — runs every second, checks tasks, sends triggers.
///
/// Stops when `cancel` is triggered, `trigger_tx` receiver is dropped,
/// or the channel-full send is interrupted by cancellation.
///
/// Two-phase locking: first acquires a read lock to check for expiry /
/// firing. When nothing needs to change (the common case), the loop
/// releases the read lock and iterates — other readers (e.g. the bridge
/// calling `list()`) never block. Only when tasks must be mutated does
/// the loop upgrade to a write lock.
///
/// ## Concurrency semantics
///
/// The read and write phases do not form an atomic transaction. Between
/// `survey_tasks` releasing the read lock and `mutate_tasks` acquiring
/// the write lock, concurrent calls to [`CronScheduler::add`] or
/// [`CronScheduler::remove`] may arrive. This is **intentional** and
/// safe:
///
/// - If a task identified as "to fire" is removed in the gap, `mutate_tasks`
///   won't find its id and skips silently — which matches the caller's
///   remove-to-cancel intent.
/// - If a task identified as "expiring" is removed, same benign outcome.
/// - Newly added tasks are never accidentally touched because mutation
///   looks up by `id`, not by index, and `add` guarantees unique ids.
///
/// `now` is captured once per tick; both phases use the same value, so
/// no task is double-counted or skipped due to clock drift within a
/// tick.
pub(crate) async fn tick_loop(
    tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    trigger_tx: tokio::sync::mpsc::Sender<ScheduledTrigger>,
    cancel: CancellationToken,
    clock: Arc<dyn Clock>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = interval.tick() => {}
            () = cancel.cancelled() => break,
        }

        let now = clock.now();
        let (needs_write, expiring_ids, firing_ids) = survey_tasks(&tasks, &now).await;
        if !needs_write {
            continue;
        }

        let triggers = mutate_tasks(&tasks, &now, &expiring_ids, &firing_ids).await;

        for trigger in triggers {
            tokio::select! {
                result = trigger_tx.send(trigger) => {
                    if result.is_err() {
                        return;
                    }
                }
                () = cancel.cancelled() => return,
            }
        }
    }
}

/// Read-only survey: identify task ids that need expiring or firing.
async fn survey_tasks(
    tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
    now: &chrono::DateTime<chrono::Utc>,
) -> (bool, Vec<String>, Vec<String>) {
    let guard = tasks.read().await;
    let mut expiring = Vec::new();
    let mut firing = Vec::new();
    for task in guard.iter() {
        if task.is_expired(now) {
            expiring.push(task.id.clone());
        } else if task.should_fire(now) {
            firing.push(task.id.clone());
        }
    }
    let needs_write = !expiring.is_empty() || !firing.is_empty();
    (needs_write, expiring, firing)
}

/// Exclusive mutation: stamp `last_fired`, build triggers, remove
/// expired/one-shot tasks. Returns the triggers to dispatch.
async fn mutate_tasks(
    tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
    now: &chrono::DateTime<chrono::Utc>,
    expiring_ids: &[String],
    firing_ids: &[String],
) -> Vec<ScheduledTrigger> {
    let mut guard = tasks.write().await;
    let mut triggers = Vec::new();
    let mut to_remove: Vec<usize> = Vec::new();

    for (i, task) in guard.iter_mut().enumerate() {
        if expiring_ids.contains(&task.id) {
            info!(task_id = %task.id, "cron job expired");
            to_remove.push(i);
        } else if firing_ids.contains(&task.id) {
            info!(task_id = %task.id, "cron job fired");
            triggers.push(ScheduledTrigger {
                task_id: task.id.clone(),
                prompt: task.prompt.clone(),
                fired_at: *now,
            });
            task.last_fired = Some(*now);
            if !task.recurring {
                to_remove.push(i);
            }
        }
    }

    // Indices are pushed in ascending order (enumerate over the vec) and
    // each task hits at most one branch (if / else if), so `to_remove` is
    // already sorted and unique — no `sort()` or `dedup()` required.
    debug_assert!(
        to_remove.windows(2).all(|w| w[0] < w[1]),
        "to_remove must be strictly ascending"
    );
    for i in to_remove.into_iter().rev() {
        guard.remove(i);
    }
    triggers
}
