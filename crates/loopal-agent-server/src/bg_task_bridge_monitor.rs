use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{AgentEventPayload, BgTaskStatus};
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_tool_background::{BackgroundTask, BackgroundTaskStore, TaskStatus};
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::bg_task_bridge_sampler::run_file_sampler;

pub(super) fn capture_source(task: &BackgroundTask) -> (PathBuf, watch::Receiver<TaskStatus>) {
    (task.log_path().to_path_buf(), task.status_watch())
}

pub(super) fn spawn_task_monitor(
    monitors: &mut JoinSet<String>,
    task_id: String,
    log_path: PathBuf,
    mut status_watch: watch::Receiver<TaskStatus>,
    store: Arc<BackgroundTaskStore>,
    frontend: &Arc<dyn AgentFrontend>,
    sample_interval: Duration,
) {
    let sampler_emitter = frontend.event_emitter();
    let watcher_emitter = frontend.event_emitter();
    let return_id = task_id.clone();
    monitors.spawn(async move {
        tokio::select! {
            _ = run_file_sampler(task_id.clone(), log_path, sampler_emitter, sample_interval) => {}
            _ = wait_for_completion(&mut status_watch) => {}
        }
        emit_completion(&task_id, &store, watcher_emitter.as_ref()).await;
        return_id
    });
}

async fn emit_completion(
    task_id: &str,
    store: &Arc<BackgroundTaskStore>,
    emitter: &dyn EventEmitter,
) {
    let Some((status_raw, code, preview)) =
        store.read_task(task_id, |t| (t.status(), t.exit_code(), t.render_preview()))
    else {
        return;
    };
    let status = match status_raw {
        TaskStatus::Completed => BgTaskStatus::Completed,
        TaskStatus::Failed => BgTaskStatus::Failed,
        TaskStatus::Killed => BgTaskStatus::Killed,
        TaskStatus::Running => BgTaskStatus::Failed,
    };
    if let Err(e) = emitter
        .emit(AgentEventPayload::BgTaskCompleted {
            id: task_id.to_string(),
            status,
            exit_code: code,
            output: preview,
        })
        .await
    {
        tracing::warn!(error = %e, label = "BgTaskCompleted", "failed to emit");
    }
}

async fn wait_for_completion(rx: &mut watch::Receiver<TaskStatus>) {
    rx.borrow_and_update();
    loop {
        if rx.changed().await.is_err() {
            return;
        }
        if rx.borrow().is_terminal() {
            return;
        }
    }
}
