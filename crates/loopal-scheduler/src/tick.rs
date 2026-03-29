//! Background tick loop — runs every second, checks tasks, sends triggers.

use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::clock::Clock;
use crate::task::ScheduledTask;
use crate::trigger::ScheduledTrigger;

/// Core tick loop — runs every second, checks tasks, sends triggers.
///
/// Stops when `cancel` is triggered, `trigger_tx` receiver is dropped,
/// or the channel-full send is interrupted by cancellation.
pub(crate) async fn tick_loop(
    tasks: Arc<Mutex<Vec<ScheduledTask>>>,
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
        let mut tasks = tasks.lock().await;
        let mut triggered_ids = Vec::new();
        let mut to_remove = Vec::new();

        for (i, task) in tasks.iter_mut().enumerate() {
            if task.is_expired(&now) {
                info!(task_id = %task.id, "cron job expired");
                to_remove.push(i);
                continue;
            }
            if task.should_fire(&now) {
                info!(task_id = %task.id, "cron job fired");
                let trigger = ScheduledTrigger {
                    task_id: task.id.clone(),
                    prompt: task.prompt.clone(),
                    fired_at: now,
                };
                triggered_ids.push((i, trigger));
                task.last_fired = Some(now);
            }
        }

        let remove_oneshots: Vec<usize> = triggered_ids
            .iter()
            .filter(|(i, _)| !tasks[*i].recurring)
            .map(|(i, _)| *i)
            .collect();
        to_remove.extend(remove_oneshots);

        to_remove.sort_unstable();
        to_remove.dedup();
        for i in to_remove.into_iter().rev() {
            tasks.remove(i);
        }

        let triggers: Vec<_> = triggered_ids.into_iter().map(|(_, t)| t).collect();
        drop(tasks);

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
