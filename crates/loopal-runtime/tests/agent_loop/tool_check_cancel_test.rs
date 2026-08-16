use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEventPayload, InterruptSignal};
use loopal_runtime::agent_loop::{StreamingToolHandle, TurnContext, cancel::TurnCancel};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use serde_json::{Value, json};

use super::{in_turn, make_runner_with_kernel};

struct CancelDuringPrecheck {
    signal: InterruptSignal,
    generation: Arc<tokio::sync::watch::Sender<u64>>,
}

#[async_trait]
impl Tool for CancelDuringPrecheck {
    fn name(&self) -> &str {
        "CancelDuringPrecheck"
    }

    fn description(&self) -> &str {
        "cancels its batch during admission"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn precheck(&self, _: &Value) -> Option<String> {
        self.signal.signal();
        self.generation
            .send_modify(|generation| *generation = generation.wrapping_add(1));
        None
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        std::future::pending().await
    }
}

#[tokio::test]
async fn cancellation_between_checks_terminalizes_remaining_calls() {
    let signal = InterruptSignal::new();
    let generation = Arc::new(tokio::sync::watch::channel(0u64).0);
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(CancelDuringPrecheck {
        signal: signal.clone(),
        generation: Arc::clone(&generation),
    }));
    let (mut runner, mut events, _) = make_runner_with_kernel(Arc::new(kernel));
    let cancel = TurnCancel::new(signal, generation);
    let mut turn_ctx = TurnContext::new(0, cancel);

    let stats = in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![
            ("first".into(), "CancelDuringPrecheck".into(), json!({})),
            ("second".into(), "CancelDuringPrecheck".into(), json!({})),
        ],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!((stats.approved, stats.denied, stats.errors), (1, 1, 2));
    let results = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event.payload {
            AgentEventPayload::ToolResult { id, metadata, .. } => Some((id, metadata)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(_, metadata)| matches!(
        metadata,
        Some(ToolResultMetadata::Cancelled {
            cause: CancelCause::UserInterrupt
        })
    )));
}
