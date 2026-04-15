//! Tests for bg_task_bridge — event-driven background task monitoring.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_tool_background::BackgroundTaskStore;

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
        (Self { events: events.clone() }, events)
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

#[tokio::test]
async fn spawned_event_emitted_on_notification() {
    let store = BackgroundTaskStore::new();
    let rx = store.subscribe_spawns();
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::bg_task_bridge_spawn(rx, Arc::new(frontend));

    store.register_proxy("bg_1".into(), "test task".into());
    tokio::time::sleep(Duration::from_millis(50)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    assert!(captured.iter().any(|e| matches!(
        e,
        AgentEventPayload::BgTaskSpawned { id, .. } if id == "bg_1"
    )));
}

#[tokio::test]
async fn completion_event_emitted_on_task_finish() {
    let store = BackgroundTaskStore::new();
    let rx = store.subscribe_spawns();
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::bg_task_bridge_spawn(rx, Arc::new(frontend));

    let handle = store.register_proxy("bg_1".into(), "finishing".into());
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.complete("final output".into(), true);
    tokio::time::sleep(Duration::from_millis(100)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let completed = captured.iter().find(|e| matches!(
        e,
        AgentEventPayload::BgTaskCompleted { id, .. } if id == "bg_1"
    ));
    assert!(completed.is_some(), "should emit BgTaskCompleted");
    if let Some(AgentEventPayload::BgTaskCompleted { output, .. }) = completed {
        assert_eq!(output, "final output");
    }
}

#[tokio::test]
async fn output_delta_emitted_for_running_task() {
    let store = BackgroundTaskStore::new();
    let rx = store.subscribe_spawns();
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::bg_task_bridge_spawn(rx, Arc::new(frontend));

    let proxy = store.register_proxy("bg_1".into(), "output test".into());
    tokio::time::sleep(Duration::from_millis(50)).await;

    store.with_task("bg_1", |task| {
        task.output.lock().unwrap().push_str("hello world\n");
    });
    // Wait for sampler tick (2s interval + generous margin for CI)
    tokio::time::sleep(Duration::from_secs(5)).await;

    proxy.complete("final".into(), true);
    tokio::time::sleep(Duration::from_millis(100)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let has_output = captured.iter().any(|e| matches!(
        e,
        AgentEventPayload::BgTaskOutput { id, output_delta }
            if id == "bg_1" && output_delta.contains("hello world")
    ));
    assert!(has_output, "should emit BgTaskOutput with delta");
}

#[tokio::test]
async fn instant_completion_skips_output_events() {
    let store = BackgroundTaskStore::new();
    let rx = store.subscribe_spawns();
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::bg_task_bridge_spawn(rx, Arc::new(frontend));

    let handle = store.register_proxy("bg_fast".into(), "instant".into());
    handle.complete("done".into(), true);
    tokio::time::sleep(Duration::from_millis(200)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let output_count = captured
        .iter()
        .filter(|e| matches!(e, AgentEventPayload::BgTaskOutput { .. }))
        .count();
    assert_eq!(output_count, 0, "instant completion should skip output");
}
