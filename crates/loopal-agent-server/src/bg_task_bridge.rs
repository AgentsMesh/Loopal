use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::traits::AgentFrontend;
use loopal_tool_background::{BackgroundTaskStore, SpawnNotification, StatusFilter};
use tokio::sync::{broadcast, oneshot};
use tokio::task::{JoinHandle, JoinSet};

use crate::bg_task_bridge_monitor::{capture_source, spawn_task_monitor};

pub fn spawn(
    mut spawn_rx: broadcast::Receiver<SpawnNotification>,
    store: Arc<BackgroundTaskStore>,
    frontend: Arc<dyn AgentFrontend>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> JoinHandle<()> {
    let sample_interval = store.config().output_sample_interval();
    tokio::spawn(async move {
        let mut monitors: JoinSet<String> = JoinSet::new();
        let mut attached: HashSet<String> = HashSet::new();
        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_rx => break,
                Some(res) = monitors.join_next(), if !monitors.is_empty() => {
                    if let Ok(task_id) = res {
                        attached.remove(&task_id);
                    }
                }
                recv = spawn_rx.recv() => match recv {
                    Ok(notif) => {
                        handle_spawn(notif, &store, &frontend, &mut monitors, &mut attached, sample_interval).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "bg spawn broadcast lagged — reconciling missed tasks");
                        reconcile_missed(&store, &frontend, &mut monitors, &mut attached, sample_interval).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
        monitors.shutdown().await;
    })
}

async fn handle_spawn(
    notif: SpawnNotification,
    store: &Arc<BackgroundTaskStore>,
    frontend: &Arc<dyn AgentFrontend>,
    monitors: &mut JoinSet<String>,
    attached: &mut HashSet<String>,
    sample_interval: Duration,
) {
    emit_spawned_and_attach(
        &notif.task_id,
        &notif.description,
        notif.created_at_unix_ms,
        store,
        frontend,
        monitors,
        attached,
        sample_interval,
        "BgTaskSpawned",
    )
    .await;
}

async fn reconcile_missed(
    store: &Arc<BackgroundTaskStore>,
    frontend: &Arc<dyn AgentFrontend>,
    monitors: &mut JoinSet<String>,
    attached: &mut HashSet<String>,
    sample_interval: Duration,
) {
    for snap in store.snapshot(StatusFilter::All) {
        if attached.contains(&snap.id) {
            continue;
        }
        emit_spawned_and_attach(
            &snap.id,
            &snap.description,
            snap.created_at_unix_ms,
            store,
            frontend,
            monitors,
            attached,
            sample_interval,
            "reconcile BgTaskSpawned",
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn emit_spawned_and_attach(
    task_id: &str,
    description: &str,
    created_at_unix_ms: u64,
    store: &Arc<BackgroundTaskStore>,
    frontend: &Arc<dyn AgentFrontend>,
    monitors: &mut JoinSet<String>,
    attached: &mut HashSet<String>,
    sample_interval: Duration,
    label: &str,
) {
    emit_or_warn(
        frontend,
        AgentEventPayload::BgTaskSpawned {
            id: task_id.to_string(),
            description: description.to_string(),
            created_at_unix_ms,
        },
        label,
    )
    .await;
    attach_monitor(
        monitors,
        task_id,
        store,
        frontend,
        attached,
        sample_interval,
    );
}

fn attach_monitor(
    monitors: &mut JoinSet<String>,
    task_id: &str,
    store: &Arc<BackgroundTaskStore>,
    frontend: &Arc<dyn AgentFrontend>,
    attached: &mut HashSet<String>,
    sample_interval: Duration,
) {
    if !attached.insert(task_id.to_string()) {
        return;
    }
    let Some((log_path, status_watch)) = store.read_task(task_id, capture_source) else {
        attached.remove(task_id);
        return;
    };
    spawn_task_monitor(
        monitors,
        task_id.to_string(),
        log_path,
        status_watch,
        store.clone(),
        frontend,
        sample_interval,
    );
}

async fn emit_or_warn(frontend: &Arc<dyn AgentFrontend>, payload: AgentEventPayload, label: &str) {
    if let Err(e) = frontend.emit(payload).await {
        tracing::warn!(error = %e, label, "failed to emit");
    }
}
