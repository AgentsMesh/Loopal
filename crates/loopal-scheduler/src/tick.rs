//! Background tick loop — runs every second, checks tasks, sends triggers.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::persistence::durable_snapshot;
use crate::scheduler::ActiveBinding;
use crate::task::ScheduledTask;
use crate::tick_context::TickContext;
use crate::trigger::ScheduledTrigger;

/// Core tick loop — every second, surveys + mutates tasks, sends triggers.
///
/// Stops on `cancel`, dropped `trigger_tx` receiver, or send-cancellation race.
///
/// **Two-phase locking** (intentional): read survey → write mutate. Concurrent
/// `add` / `remove` between phases is safe (see helper docs).
///
/// **Persistence**: post-mutation snapshot of durable tasks is saved while the
/// write lock is still held — at most one save per tick. Retries fire on
/// `dirty == true` even when no mutations occurred this tick.
pub(crate) async fn tick_loop(
    ctx: TickContext,
    trigger_tx: tokio::sync::mpsc::Sender<ScheduledTrigger>,
    cancel: CancellationToken,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = interval.tick() => {}
            () = cancel.cancelled() => break,
        }

        let now = ctx.clock.now();
        let (needs_write, expiring_ids, firing_ids) = survey_tasks(&ctx.tasks, &now).await;
        let should_retry = ctx.dirty.load(Ordering::Acquire);
        if !needs_write && !should_retry {
            continue;
        }

        // Treat a disabled store as "no binding" for this tick so the
        // retry path doesn't thrash trying to save into a file that
        // `load_persisted` already refused to normalize.
        let resolved_binding = if ctx.store_disabled.load(Ordering::Acquire) {
            None
        } else {
            resolve_active(&ctx.active).await
        };
        let triggers = mutate_tasks(
            &ctx.tasks,
            &now,
            &expiring_ids,
            &firing_ids,
            resolved_binding,
            &ctx.dirty,
        )
        .await;

        // If anything was removed (one-shot fire, expiry) or fired (a
        // recurring task's `last_fired` advanced), the job set's
        // user-visible state changed — broadcast so observers re-snapshot.
        // No-op when no receivers are attached.
        if needs_write {
            let _ = ctx.change_tx.send(());
        }

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

/// Snapshot the current `(storage, session_id)` from `active`, or `None`
/// if either is unset. Cloning out lets `mutate_tasks` await the save
/// without holding the active mutex while the storage I/O runs.
async fn resolve_active(active: &Arc<Mutex<Option<ActiveBinding>>>) -> Option<ResolvedBinding> {
    let guard = active.lock().await;
    let binding = guard.as_ref()?;
    let session_id = binding.session_id.as_ref()?.clone();
    Some(ResolvedBinding {
        storage: binding.storage.clone(),
        session_id,
    })
}

struct ResolvedBinding {
    storage: Arc<dyn crate::persistence_session::SessionScopedCronStorage>,
    session_id: String,
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
/// expired/one-shot tasks, then persist if any durable task changed
/// (or if the previous save failed). Returns the triggers to dispatch.
async fn mutate_tasks(
    tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
    now: &chrono::DateTime<chrono::Utc>,
    expiring_ids: &[String],
    firing_ids: &[String],
    binding: Option<ResolvedBinding>,
    dirty: &Arc<AtomicBool>,
) -> Vec<ScheduledTrigger> {
    let mut guard = tasks.write().await;
    let mut triggers = Vec::new();
    let mut to_remove: Vec<usize> = Vec::new();
    let mut durable_touched = false;

    for (i, task) in guard.iter_mut().enumerate() {
        if expiring_ids.contains(&task.id) {
            info!(task_id = %task.id, "cron job expired");
            if task.durable {
                durable_touched = true;
            }
            to_remove.push(i);
        } else if firing_ids.contains(&task.id) {
            info!(task_id = %task.id, "cron job fired");
            triggers.push(ScheduledTrigger {
                task_id: task.id.clone(),
                prompt: task.prompt.clone(),
                fired_at: *now,
            });
            task.last_fired = Some(*now);
            if task.durable {
                durable_touched = true;
            }
            if !task.recurring {
                to_remove.push(i);
            }
        }
    }

    debug_assert!(
        to_remove.windows(2).all(|w| w[0] < w[1]),
        "to_remove must be strictly ascending"
    );
    for i in to_remove.into_iter().rev() {
        guard.remove(i);
    }

    if let Some(b) = binding {
        let retry_pending = dirty.load(Ordering::Acquire);
        if durable_touched || retry_pending {
            let snapshot = durable_snapshot(&guard);
            match b.storage.save_all(&b.session_id, &snapshot).await {
                Ok(()) => dirty.store(false, Ordering::Release),
                Err(e) => {
                    tracing::error!(error = %e, "cron durable save failed in tick");
                    dirty.store(true, Ordering::Release);
                }
            }
        }
    }
    triggers
}
