use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_background::BackgroundTaskStore;
use loopal_tool_bash::{BashParams, BashTool};
use serde_json::json;
use tokio::sync::oneshot;

// reason: hardcoded "test-session" was racy in CI — every test in this
// binary creates `/tmp/loopal/test-session/bash/{uuid}.log` files, and
// the 70-task reconcile test stresses fd limits. Each test now gets a
// pid+counter-scoped id so they no longer share the bash log dir.
fn unique_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

struct CaptureEmitter {
    events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

impl CaptureEmitter {
    fn new(events: Arc<Mutex<Vec<AgentEventPayload>>>) -> Self {
        Self { events }
    }
}

#[async_trait]
impl EventEmitter for CaptureEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
}

struct CaptureFrontend {
    events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

impl CaptureFrontend {
    fn new() -> (Self, Arc<Mutex<Vec<AgentEventPayload>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: events.clone(),
            },
            events,
        )
    }
}

#[async_trait]
impl loopal_runtime::frontend::traits::AgentFrontend for CaptureFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
    async fn recv_input(&self) -> Option<loopal_runtime::agent_input::AgentInput> {
        None
    }
    async fn try_recv_input(
        &self,
    ) -> std::result::Result<
        loopal_runtime::agent_input::AgentInput,
        tokio::sync::mpsc::error::TryRecvError,
    > {
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    }
    async fn request_permission(
        &self,
        _id: &str,
        _name: &str,
        _input: &serde_json::Value,
    ) -> loopal_tool_api::PermissionDecision {
        loopal_tool_api::PermissionDecision::Allow
    }
    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(CaptureEmitter::new(self.events.clone()))
    }
}

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        unique_session_id(),
    );
    ToolContext::new(backend, "test")
}

async fn bash_spawn(store: &Arc<BackgroundTaskStore>, cmd: &str) -> String {
    try_bash_spawn(store, cmd)
        .await
        .expect("single-spawn tests must succeed; bump fd limits if this flakes")
}

// Lenient variant for stress tests that intentionally exceed broadcast/fd
// limits. Returns None when `exec_background` fails (e.g. transient
// EMFILE under 70-spawn fan-out) so the caller can tolerate partial
// failure instead of panicking inside the loop.
async fn try_bash_spawn(store: &Arc<BackgroundTaskStore>, cmd: &str) -> Option<String> {
    let bash: TypedBridge<BashTool, BashParams> = TypedBridge::new(BashTool::new(store.clone()));
    let ctx = make_ctx(&std::env::temp_dir());
    let result = bash
        .execute(json!({"command": cmd, "run_in_background": true}), &ctx)
        .await
        .ok()?;
    result
        .content
        .lines()
        .find_map(|l| l.strip_prefix("process_id: "))
        .map(str::to_string)
}

fn spawn_bridge(
    store: &Arc<BackgroundTaskStore>,
    frontend: Arc<CaptureFrontend>,
) -> (tokio::task::JoinHandle<()>, oneshot::Sender<()>) {
    let rx = store.subscribe_spawns();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = loopal_agent_server::testing::bg_task_bridge_spawn(
        rx,
        store.clone(),
        frontend,
        shutdown_rx,
    );
    (task, shutdown_tx)
}

async fn shutdown_bridge(task: tokio::task::JoinHandle<()>, tx: oneshot::Sender<()>) {
    let _ = tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
}

#[tokio::test]
async fn spawned_event_emitted_on_notification() {
    let store = BackgroundTaskStore::new();
    let (frontend, events) = CaptureFrontend::new();
    let (bridge, shutdown) = spawn_bridge(&store, Arc::new(frontend));

    let pid = bash_spawn(&store, "echo hi").await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    shutdown_bridge(bridge, shutdown).await;

    let captured = events.lock().unwrap();
    assert!(captured.iter().any(|e| matches!(
        e,
        AgentEventPayload::BgTaskSpawned { id, .. } if id == &pid
    )));
}

#[tokio::test]
#[cfg(not(windows))]
async fn completion_event_emitted_on_task_finish() {
    let store = BackgroundTaskStore::new();
    let (frontend, events) = CaptureFrontend::new();
    let (bridge, shutdown) = spawn_bridge(&store, Arc::new(frontend));

    let pid = bash_spawn(&store, "printf %s final_output").await;
    tokio::time::sleep(Duration::from_millis(180)).await;
    shutdown_bridge(bridge, shutdown).await;

    let captured = events.lock().unwrap();
    let completed = captured.iter().find(|e| {
        matches!(
            e,
            AgentEventPayload::BgTaskCompleted { id, .. } if id == &pid
        )
    });
    assert!(completed.is_some(), "should emit BgTaskCompleted");
    if let Some(AgentEventPayload::BgTaskCompleted { output, .. }) = completed {
        assert!(
            output.contains("final_output"),
            "completion output must include process stdout, got: {output}"
        );
    }
}

#[tokio::test]
async fn graceful_shutdown_signal_stops_bridge() {
    let store = BackgroundTaskStore::new();
    let (frontend, _events) = CaptureFrontend::new();
    let (bridge, shutdown) = spawn_bridge(&store, Arc::new(frontend));

    let _ = shutdown.send(());
    let joined = tokio::time::timeout(Duration::from_secs(1), bridge).await;
    assert!(joined.is_ok(), "bridge should exit cleanly on shutdown");
}

#[tokio::test]
async fn reconcile_missed_re_emits_tasks_when_broadcast_lagged() {
    // reason: pre-subscribe to broadcast (capturing the receiver) but never
    // consume; spawn > SPAWN_BROADCAST_CAP (64) tasks so the channel
    // overflows. Then hand that lagged receiver to bridge::spawn — its first
    // recv() must surface Lagged, triggering reconcile_missed which re-emits
    // every in-store task via snapshot(All).
    use loopal_tool_background::SpawnNotification;
    use tokio::sync::{broadcast, oneshot};

    let store = BackgroundTaskStore::new();
    let rx: broadcast::Receiver<SpawnNotification> = store.subscribe_spawns();

    let mut pids = Vec::new();
    for _ in 0..70 {
        if let Some(pid) = try_bash_spawn(&store, "true").await {
            pids.push(pid);
        }
    }
    // Need enough tasks in the store to actually exceed the broadcast cap
    // (64); otherwise the test premise (lagged Reconcile) doesn't hold.
    assert!(
        pids.len() >= 65,
        "stress test premise broken: only {} of 70 spawns succeeded; \
         likely fd exhaustion in this test binary",
        pids.len(),
    );

    let (frontend, events) = CaptureFrontend::new();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let bridge = loopal_agent_server::testing::bg_task_bridge_spawn(
        rx,
        store.clone(),
        Arc::new(frontend),
        shutdown_rx,
    );

    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), bridge).await;

    let captured = events.lock().unwrap();
    let spawn_ids: Vec<String> = captured
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::BgTaskSpawned { id, .. } if id.starts_with("bg_") => {
                Some(id.clone())
            }
            _ => None,
        })
        .collect();
    let min_expected = (pids.len() * 9) / 10;
    assert!(
        spawn_ids.len() >= min_expected,
        "reconcile should re-emit majority ({} of {} pre-existing tasks) (got {}): {:?}",
        min_expected,
        pids.len(),
        spawn_ids.len(),
        spawn_ids
    );
}

#[tokio::test]
#[cfg(not(windows))]
async fn file_sampler_emits_output_delta_for_process_task() {
    use loopal_tool_api::OutputTail;

    let store_arc = BackgroundTaskStore::with_config(loopal_tool_api::BgTaskConfig {
        output_sample_interval_secs: 1,
        ..loopal_tool_api::BgTaskConfig::default()
    });
    let (frontend, events) = CaptureFrontend::new();
    let (bridge, shutdown) = {
        let rx = store_arc.subscribe_spawns();
        let (tx, rx2) = oneshot::channel();
        let task = loopal_agent_server::testing::bg_task_bridge_spawn(
            rx,
            store_arc.clone(),
            Arc::new(frontend),
            rx2,
        );
        (task, tx)
    };

    let tmp = tempfile::tempdir().unwrap();
    let backend = loopal_backend::LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        unique_session_id(),
    );
    let ctx = ToolContext::new(backend, "test").with_output_tail(Arc::new(OutputTail::new(20)));
    let bridge_tool: TypedBridge<BashTool, BashParams> =
        TypedBridge::new(BashTool::new(store_arc.clone()));

    let result = bridge_tool
        .execute(
            json!({
                "command": "for i in 1 2 3; do echo line_$i; sleep 0.4; done",
                "run_in_background": true
            }),
            &ctx,
        )
        .await
        .unwrap();
    let pid = result
        .content
        .lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap()
        .to_string();

    tokio::time::sleep(Duration::from_millis(5000)).await;
    shutdown_bridge(bridge, shutdown).await;

    let captured = events.lock().unwrap();
    let outputs: Vec<&str> = captured
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::BgTaskOutput { id, output_delta } if id == &pid => {
                Some(output_delta.as_str())
            }
            AgentEventPayload::BgTaskCompleted { id, output, .. } if id == &pid => {
                Some(output.as_str())
            }
            _ => None,
        })
        .collect();
    let merged: String = outputs.concat();
    assert!(
        merged.contains("line_1"),
        "expected output to contain line_1: {merged}"
    );
    assert!(
        merged.contains("line_3"),
        "expected output to contain line_3: {merged}"
    );
}
