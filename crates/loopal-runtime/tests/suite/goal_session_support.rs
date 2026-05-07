use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::goal::GoalRuntimeSession;
use loopal_storage::GoalStore;
use tempfile::TempDir;

#[derive(Default, Clone)]
pub struct CapturingEmitter {
    pub events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

#[async_trait]
impl EventEmitter for CapturingEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
}

pub fn fixture() -> (
    TempDir,
    Arc<GoalStore>,
    CapturingEmitter,
    GoalRuntimeSession,
) {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(GoalStore::with_base_dir(tmp.path().to_path_buf()));
    let emitter = CapturingEmitter::default();
    let session = GoalRuntimeSession::new(
        "sess".to_string(),
        Arc::clone(&store),
        Box::new(emitter.clone()),
    );
    (tmp, store, emitter, session)
}

pub fn last_payload(emitter: &CapturingEmitter) -> AgentEventPayload {
    emitter.events.lock().unwrap().last().unwrap().clone()
}
