//! Shared test helpers for cron_bridge_test and cron_bridge_edge_test.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use loopal_error::{LoopalError, Result};
use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::traits::EventEmitter;

pub struct CaptureEmitter {
    pub events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

#[async_trait]
impl EventEmitter for CaptureEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
}

pub struct CaptureFrontend {
    pub events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

impl CaptureFrontend {
    pub fn new() -> (Self, Arc<Mutex<Vec<AgentEventPayload>>>) {
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
    async fn request_permission(
        &self,
        _id: &str,
        _name: &str,
        _input: &serde_json::Value,
    ) -> loopal_tool_api::PermissionDecision {
        loopal_tool_api::PermissionDecision::Allow
    }
    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(CaptureEmitter {
            events: self.events.clone(),
        })
    }
}

/// Frontend that always returns an error from `emit` — exercises the warn
/// logging path in the bridge without crashing the loop.
pub struct FailingFrontend {
    pub emit_calls: Arc<AtomicUsize>,
}

impl FailingFrontend {
    pub fn new() -> (Self, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (
            Self {
                emit_calls: count.clone(),
            },
            count,
        )
    }
}

#[async_trait]
impl loopal_runtime::frontend::traits::AgentFrontend for FailingFrontend {
    async fn emit(&self, _payload: AgentEventPayload) -> Result<()> {
        self.emit_calls.fetch_add(1, Ordering::SeqCst);
        Err(LoopalError::Other("test-forced error".into()))
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
        panic!("unused in this test")
    }
}

pub fn count_cron_events(events: &[AgentEventPayload]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, AgentEventPayload::CronsChanged { .. }))
        .count()
}

pub fn last_cron_ids(events: &[AgentEventPayload]) -> Option<Vec<String>> {
    events.iter().rev().find_map(|e| match e {
        AgentEventPayload::CronsChanged { crons } => {
            Some(crons.iter().map(|c| c.id.clone()).collect())
        }
        _ => None,
    })
}
