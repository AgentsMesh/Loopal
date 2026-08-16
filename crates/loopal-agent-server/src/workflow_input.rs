use std::collections::HashMap;
use std::sync::Arc;

use loopal_agent::ProactiveWorkflowPlanner;
use loopal_agent::workflow_control::{WorkflowControlClient, WorkflowStartControlError};
use loopal_protocol::{
    Envelope, WorkflowExecution, WorkflowRequestId, WorkflowStartLookupRequest,
    WorkflowStartLookupResponse, WorkflowStartRequest,
};
use loopal_runtime::workflow_input::{WorkflowInputDisposition, WorkflowInputHandler};
use tokio::sync::Mutex;

const MAX_CACHED_DECISIONS: usize = 256;

#[path = "workflow_input_cache.rs"]
mod cache;
use cache::DecisionCache;

/// Adapter between the runtime's dependency-inverted input hook and the
/// agent-layer planner/Hub control client. The bounded cache makes concurrent
/// and recent duplicate deliveries single-flight. After eviction, the stable
/// request id and Hub's durable payload ledger remain the workflow-start
/// idempotency boundary. A cache miss checks that ledger before replanning.
pub(crate) struct ProactiveWorkflowInputHandler {
    planner: ProactiveWorkflowPlanner,
    control: Arc<dyn WorkflowControlClient>,
    decisions: Mutex<DecisionCache>,
    pending_starts: Mutex<HashMap<uuid::Uuid, WorkflowStartRequest>>,
    /// A duplicate envelope can arrive concurrently before the first planner
    /// call has completed. The runtime consumes one envelope at a time, so a
    /// single operation lock is sufficient to make external duplicate calls
    /// single-flight without retaining an unbounded per-envelope map. If the
    /// owner is cancelled, the guard is released and a later delivery can
    /// retry with the same idempotency key.
    operation_lock: Mutex<()>,
}

impl ProactiveWorkflowInputHandler {
    pub(crate) fn new(
        planner: ProactiveWorkflowPlanner,
        control: Arc<dyn WorkflowControlClient>,
    ) -> Self {
        Self {
            planner,
            control,
            decisions: Mutex::new(DecisionCache::new(MAX_CACHED_DECISIONS)),
            pending_starts: Mutex::new(HashMap::new()),
            operation_lock: Mutex::new(()),
        }
    }

    async fn resolve_start(
        &self,
        envelope_id: uuid::Uuid,
        request: WorkflowStartRequest,
        confirmation_only: bool,
    ) -> Result<WorkflowInputDisposition, String> {
        let request_id = request.request_id.clone();
        let outcome = if confirmation_only {
            self.control.confirm_start(request).await
        } else {
            self.control.start_with_confirmation(request).await
        };
        let disposition = match outcome {
            Ok(_) => WorkflowInputDisposition::Handled,
            Err(WorkflowStartControlError::Rejected(reason)) => {
                match self.lookup_existing_start(request_id.clone()).await? {
                    true => WorkflowInputDisposition::Handled,
                    false => {
                        tracing::warn!(%reason, "workflow start rejected; using direct execution");
                        WorkflowInputDisposition::Direct
                    }
                }
            }
            Err(error @ WorkflowStartControlError::Indeterminate { .. }) => {
                return Err(error.to_string());
            }
        };
        self.pending_starts.lock().await.remove(&envelope_id);
        self.decisions.lock().await.insert(envelope_id, disposition);
        Ok(disposition)
    }

    async fn lookup_existing_start(&self, request_id: WorkflowRequestId) -> Result<bool, String> {
        let lookup = self
            .control
            .lookup_start(WorkflowStartLookupRequest {
                request_id: request_id.clone(),
            })
            .await
            .map_err(|error| {
                format!("workflow start lookup for request_id {request_id} failed: {error}")
            })?;
        match lookup {
            WorkflowStartLookupResponse::NotFound => Ok(false),
            WorkflowStartLookupResponse::Found { .. } => Ok(true),
            WorkflowStartLookupResponse::Conflict => Err(format!(
                "workflow request_id {request_id} belongs to a different operation"
            )),
        }
    }
}

impl WorkflowInputHandler for ProactiveWorkflowInputHandler {
    fn handle<'a>(
        &'a self,
        envelope: &'a Envelope,
        recent_context: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WorkflowInputDisposition, String>> + Send + 'a>,
    > {
        Box::pin(async move {
            let envelope_id = envelope.id;
            if let Some(decision) = self.decisions.lock().await.get(&envelope_id) {
                return Ok(decision);
            }
            let _operation_guard = self.operation_lock.lock().await;
            // Another caller may have completed while this one waited.
            if let Some(decision) = self.decisions.lock().await.get(&envelope_id) {
                return Ok(decision);
            }
            let request_id = WorkflowRequestId::new(format!("human_{}", envelope.id.simple()));
            if self.lookup_existing_start(request_id.clone()).await? {
                self.pending_starts.lock().await.remove(&envelope_id);
                let disposition = WorkflowInputDisposition::Handled;
                self.decisions.lock().await.insert(envelope_id, disposition);
                return Ok(disposition);
            }
            let pending_request = { self.pending_starts.lock().await.get(&envelope_id).cloned() };
            if let Some(request) = pending_request {
                return self.resolve_start(envelope_id, request, true).await;
            }
            let decision = self
                .planner
                .plan(&envelope.content.text, recent_context)
                .await;
            let disposition = match decision.execution {
                WorkflowExecution::Direct { .. } => WorkflowInputDisposition::Direct,
                WorkflowExecution::Workflow { spec } => {
                    let request = WorkflowStartRequest { request_id, spec };
                    {
                        let mut pending = self.pending_starts.lock().await;
                        if pending.len() >= MAX_CACHED_DECISIONS {
                            return Err("too many indeterminate workflow starts".into());
                        }
                        pending.insert(envelope_id, request.clone());
                    }
                    return self.resolve_start(envelope_id, request, false).await;
                }
            };
            self.decisions.lock().await.insert(envelope_id, disposition);
            Ok(disposition)
        })
    }
}

#[cfg(test)]
#[path = "workflow_input_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "workflow_input_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workflow_input_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
#[path = "workflow_input_replay_tests.rs"]
mod replay_tests;
