//! Event-driven background task bridge — subscribes to store spawn
//! notifications and emits lifecycle + output events via the Hub.
//! Per-task monitoring: output sampler + completion watcher run as
//! concurrent branches of a single `select!`, ensuring clean cancellation.
//! All per-task tasks are tracked in a JoinSet — bridge drop cascades.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::{JoinHandle, JoinSet};

use loopal_protocol::{AgentEventPayload, BgTaskStatus};
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_tool_background::{SpawnNotification, TaskStatus};

const OUTPUT_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

pub fn spawn(
    mut spawn_rx: mpsc::UnboundedReceiver<SpawnNotification>,
    frontend: Arc<dyn AgentFrontend>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut monitors = JoinSet::new();
        while let Some(notif) = spawn_rx.recv().await {
            if let Err(e) = frontend
                .emit(AgentEventPayload::BgTaskSpawned {
                    id: notif.task_id.clone(),
                    description: notif.description.clone(),
                })
                .await
            {
                tracing::warn!(error = %e, "failed to emit BgTaskSpawned");
            }
            spawn_task_monitor(
                &mut monitors,
                notif.task_id,
                notif.output,
                notif.exit_code,
                notif.status_watch,
                frontend.event_emitter(),
                frontend.event_emitter(),
            );
        }
        monitors.shutdown().await;
    })
}

fn spawn_task_monitor(
    monitors: &mut JoinSet<()>,
    task_id: String,
    output: Arc<Mutex<String>>,
    exit_code: Arc<Mutex<Option<i32>>>,
    status_watch: watch::Receiver<TaskStatus>,
    sampler_emitter: Box<dyn EventEmitter>,
    watcher_emitter: Box<dyn EventEmitter>,
) {
    let sampler_id = task_id.clone();
    let sampler_output = output.clone();

    monitors.spawn(async move {
        let mut watch = status_watch;
        tokio::select! {
            _ = run_output_sampler(sampler_id, sampler_output, sampler_emitter) => {}
            _ = wait_for_completion(&mut watch) => {}
        }
        let final_output = read_output(&output);
        let code = read_exit_code(&exit_code);
        let final_status = watch.borrow().clone();
        let status = match final_status {
            TaskStatus::Completed => BgTaskStatus::Completed,
            TaskStatus::Failed => BgTaskStatus::Failed,
            TaskStatus::Running => BgTaskStatus::Failed,
        };
        if let Err(e) = watcher_emitter
            .emit(AgentEventPayload::BgTaskCompleted {
                id: task_id,
                status,
                exit_code: code,
                output: final_output,
            })
            .await
        {
            tracing::warn!(error = %e, "failed to emit BgTaskCompleted");
        }
    });
}

async fn run_output_sampler(
    task_id: String,
    output: Arc<Mutex<String>>,
    emitter: Box<dyn EventEmitter>,
) {
    let mut last_len = 0usize;
    let mut interval = tokio::time::interval(OUTPUT_SAMPLE_INTERVAL);
    interval.tick().await;
    loop {
        interval.tick().await;
        let current = read_output(&output);
        if current.len() <= last_len {
            continue;
        }
        let delta = current[last_len..].to_string();
        last_len = current.len();
        if let Err(e) = emitter
            .emit(AgentEventPayload::BgTaskOutput {
                id: task_id.clone(),
                output_delta: delta,
            })
            .await
        {
            tracing::warn!(error = %e, "failed to emit BgTaskOutput");
        }
    }
}

async fn wait_for_completion(rx: &mut watch::Receiver<TaskStatus>) {
    rx.borrow_and_update();
    loop {
        if rx.changed().await.is_err() {
            return;
        }
        if *rx.borrow() != TaskStatus::Running {
            return;
        }
    }
}

fn read_output(output: &Arc<Mutex<String>>) -> String {
    match output.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn read_exit_code(exit_code: &Arc<Mutex<Option<i32>>>) -> Option<i32> {
    match exit_code.lock() {
        Ok(guard) => *guard,
        Err(poisoned) => *poisoned.into_inner(),
    }
}
