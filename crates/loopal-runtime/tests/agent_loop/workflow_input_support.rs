use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use loopal_protocol::Envelope;
use loopal_runtime::LifecycleMode;
use loopal_runtime::workflow_input::{WorkflowInputDisposition, WorkflowInputHandler};
use loopal_test_support::HarnessBuilder;

struct InputHandlerStub {
    outcomes: Mutex<VecDeque<Result<WorkflowInputDisposition, String>>>,
    calls: Arc<AtomicUsize>,
}

impl WorkflowInputHandler for InputHandlerStub {
    fn handle<'a>(
        &'a self,
        _envelope: &'a Envelope,
        _recent_context: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WorkflowInputDisposition, String>> + Send + 'a>,
    > {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let outcome = self
            .outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err("workflow input disposition script exhausted".into()));
        Box::pin(std::future::ready(outcome))
    }
}

pub(super) async fn harness(
    dispositions: Vec<WorkflowInputDisposition>,
    provider_calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (loopal_test_support::IntegrationHarness, Arc<AtomicUsize>) {
    harness_with_results(
        LifecycleMode::Ephemeral,
        dispositions.into_iter().map(Ok).collect(),
        provider_calls,
    )
    .await
}

pub(super) async fn harness_with_results(
    lifecycle: LifecycleMode,
    outcomes: Vec<Result<WorkflowInputDisposition, String>>,
    provider_calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> (loopal_test_support::IntegrationHarness, Arc<AtomicUsize>) {
    let mut harness = HarnessBuilder::new()
        .messages(Vec::new())
        .calls(provider_calls)
        .lifecycle(lifecycle)
        .build()
        .await;
    let calls = Arc::new(AtomicUsize::new(0));
    harness.runner.params.workflow_input_handler = Some(Arc::new(InputHandlerStub {
        outcomes: Mutex::new(outcomes.into()),
        calls: calls.clone(),
    }));
    (harness, calls)
}
