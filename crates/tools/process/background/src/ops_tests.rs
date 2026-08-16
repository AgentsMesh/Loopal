use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use loopal_backend::shell::exec_background;
use loopal_tool_api::{BgTaskConfig, EnvOverride};
use tokio::sync::{mpsc, watch};

use super::{BackgroundTaskStore, bg_output, bg_stop};
use crate::control::{ControlSignal, StopOutcome, TaskStatus};
use crate::store::CONTROL_QUEUE_CAP;
use crate::task::{BackgroundTask, SENTINEL_NO_EXIT, TaskCommon};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

struct SyntheticTask {
    store: Arc<BackgroundTaskStore>,
    control: Option<mpsc::Receiver<ControlSignal>>,
    spawned: loopal_backend::SpawnedChild,
    capture_task: loopal_backend::ProcessCaptureTask,
}

async fn synthetic_task(stop_ack_timeout_secs: u64) -> SyntheticTask {
    let session = format!(
        "ops-unit-{}-{}",
        std::process::id(),
        SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let data = exec_background(
        &std::env::temp_dir(),
        None,
        "sleep 30",
        &EnvOverride::default(),
        &session,
    )
    .await
    .unwrap();
    let store = BackgroundTaskStore::with_config(BgTaskConfig {
        stop_ack_timeout_secs,
        ..BgTaskConfig::default()
    });
    let (_status_tx, status_watch) = watch::channel(TaskStatus::Running);
    let (control_tx, control) = mpsc::channel(CONTROL_QUEUE_CAP);
    let task = BackgroundTask {
        common: TaskCommon {
            id: "synthetic".into(),
            description: "synthetic".into(),
            status_watch,
            exit_code: Arc::new(AtomicI32::new(SENTINEL_NO_EXIT)),
            created_at: Instant::now(),
            created_at_unix_ms: 0,
        },
        control_tx,
        log_path: data.log_path,
        capture_state: data.capture_state,
    };
    assert_eq!(task.log_path(), task.log_path.as_path());
    assert!(task.render_preview().contains("[full log:"));
    store.insert(task);
    SyntheticTask {
        store,
        control: Some(control),
        spawned: data.spawned,
        capture_task: data.capture_task,
    }
}

async fn cleanup(mut fixture: SyntheticTask) {
    drop(fixture.control.take());
    fixture.spawned.terminate(Duration::from_millis(100)).await;
    loopal_backend::process_capture_task::join_bounded(
        fixture.capture_task,
        Duration::from_secs(3),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn stop_reports_closed_control_channel() {
    let mut fixture = synthetic_task(1).await;
    drop(fixture.control.take());

    let result = bg_stop(&fixture.store, "synthetic").await;
    assert!(result.is_error && result.content.contains("transitioned to terminal"));
    cleanup(fixture).await;
}

#[tokio::test]
async fn stop_propagates_explicit_kill_failure() {
    let mut fixture = synthetic_task(1).await;
    let mut control = fixture.control.take().unwrap();
    let responder = tokio::spawn(async move {
        let ControlSignal::Stop { ack } = control.recv().await.unwrap();
        ack.send(StopOutcome::KillFailed("injected".into()))
            .unwrap();
    });

    let result = bg_stop(&fixture.store, "synthetic").await;
    assert!(result.is_error && result.content.contains("injected"));
    responder.await.unwrap();
    cleanup(fixture).await;
}

#[tokio::test]
async fn stop_reports_dropped_acknowledgement() {
    let mut fixture = synthetic_task(1).await;
    let mut control = fixture.control.take().unwrap();
    let responder = tokio::spawn(async move {
        let ControlSignal::Stop { ack } = control.recv().await.unwrap();
        drop(ack);
    });

    let result = bg_stop(&fixture.store, "synthetic").await;
    assert!(result.is_error && result.content.contains("channel dropped"));
    responder.await.unwrap();
    cleanup(fixture).await;
}

#[tokio::test]
async fn stop_reports_acknowledgement_timeout() {
    let fixture = synthetic_task(0).await;

    let result = bg_stop(&fixture.store, "synthetic").await;
    assert!(result.is_error && result.content.contains("timed out"));
    cleanup(fixture).await;
}

#[tokio::test]
async fn blocking_output_returns_when_the_status_sender_is_gone() {
    let fixture = synthetic_task(1).await;

    let result = bg_output(&fixture.store, "synthetic", true, Duration::from_secs(1)).await;

    assert!(!result.is_error);
    assert!(result.content.contains("Status: Running"));
    cleanup(fixture).await;
}
