use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, RwLock, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::persistence::durable_snapshot;
use crate::save_worker::{SaveMessage, SaveRequest};
use crate::scheduler::ActiveBinding;
use crate::task::ScheduledTask;
use crate::tick_context::TickContext;
use crate::trigger::ScheduledTrigger;

/// Two-phase locking (intentional): read survey → write mutate.
/// Concurrent `add` / `remove` between phases is safe — the id-set
/// snapshot taken in survey is matched against ids in mutate, so a
/// task removed between phases is simply skipped.
///
/// Persistence: post-mutation snapshot of durable tasks is pushed to
/// the save-worker channel while still under `tasks.write`. The worker
/// runs `save_all` (fsync) outside any task lock, so concurrent
/// `list`/`add`/`remove` never wait on disk I/O. At most one save
/// request per tick. Retries fire on `dirty == true` even when no
/// mutations occurred.
///
/// Stops on `cancel`, dropped `trigger_tx` receiver, or send-cancel race.
pub(crate) async fn tick_loop(
    ctx: TickContext,
    trigger_tx: mpsc::Sender<ScheduledTrigger>,
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
        let (needs_write, firing_ids) = survey_tasks(&ctx.tasks, &now).await;
        let should_retry = ctx.dirty.load(Ordering::Acquire);
        if !needs_write && !should_retry {
            continue;
        }

        // Treat a disabled store as "no binding" so the retry path
        // doesn't thrash against a file `load_persisted` already refused.
        let resolved_binding = if ctx.store_disabled.load(Ordering::Acquire) {
            None
        } else {
            resolve_active(&ctx.active).await
        };
        let triggers = mutate_tasks(
            &ctx.tasks,
            &now,
            &firing_ids,
            resolved_binding,
            &ctx.dirty,
            &ctx.store_disabled,
            &ctx.save_tx,
        )
        .await;

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

async fn survey_tasks(
    tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
    now: &chrono::DateTime<chrono::Utc>,
) -> (bool, HashSet<String>) {
    let guard = tasks.read().await;
    let mut firing = HashSet::new();
    for task in guard.iter() {
        if task.should_fire(now) {
            firing.insert(task.id.clone());
        }
    }
    let needs_write = !firing.is_empty();
    (needs_write, firing)
}

#[allow(clippy::too_many_arguments)]
async fn mutate_tasks(
    tasks: &Arc<RwLock<Vec<ScheduledTask>>>,
    now: &chrono::DateTime<chrono::Utc>,
    firing_ids: &HashSet<String>,
    binding: Option<ResolvedBinding>,
    dirty: &Arc<AtomicBool>,
    store_disabled: &Arc<AtomicBool>,
    save_tx: &mpsc::Sender<SaveMessage>,
) -> Vec<ScheduledTrigger> {
    let mut guard = tasks.write().await;
    let mut triggers = Vec::new();
    let mut to_remove: Vec<usize> = Vec::new();
    let mut durable_touched = false;

    for (i, task) in guard.iter_mut().enumerate() {
        if firing_ids.contains(&task.id) {
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
            // Drop the tasks write lock before queuing the save so the
            // worker's I/O cannot block other mutations.
            drop(guard);
            let req = SaveRequest {
                storage: b.storage,
                session_id: b.session_id,
                snapshot,
                dirty: dirty.clone(),
                store_disabled: store_disabled.clone(),
            };
            // try_send (not send().await) mirrors `persist_locked` — a
            // backed-up worker shouldn't extend the tick interval; the
            // dirty flag will trigger a retry on the next tick once the
            // worker drains.
            match save_tx.try_send(SaveMessage::Save(req)) {
                Ok(()) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!("cron save worker queue full; deferring to next tick");
                    dirty.store(true, Ordering::Release);
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    tracing::error!(
                        "cron save worker channel closed; latching store_disabled to stop retries"
                    );
                    store_disabled.store(true, Ordering::Release);
                }
            }
        }
    }
    triggers
}
