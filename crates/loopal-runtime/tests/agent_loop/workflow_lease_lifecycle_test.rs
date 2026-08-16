use std::sync::Arc;

use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_protocol::{
    Envelope, MessageSource, WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalDisposition, WorkflowTerminalNotification, WorkflowTerminalOutcome,
};
use loopal_provider_api::Provider;
use loopal_runtime::agent_input::{AgentInput, WorkflowTerminalRequest};
use loopal_runtime::workflow_input::{WorkflowInputDisposition, WorkflowInputHandler};
use loopal_runtime::{LifecycleMode, WorkflowLeaseTracker};
use loopal_test_support::chunks;

use super::make_idle_runner_with_tracked_kernel;
use super::mock_provider::MultiCallProvider;

struct StartsWorkflow {
    tracker: Arc<WorkflowLeaseTracker>,
    run_id: WorkflowRunId,
    inputs: tokio::sync::mpsc::Sender<AgentInput>,
    terminal: WorkflowTerminalRequest,
}

impl WorkflowInputHandler for StartsWorkflow {
    fn handle<'a>(
        &'a self,
        _envelope: &'a Envelope,
        _recent_context: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WorkflowInputDisposition, String>> + Send + 'a>,
    > {
        let tracked = self.tracker.track(self.run_id.clone());
        let inputs = self.inputs.clone();
        let terminal = self.terminal.clone();
        Box::pin(async move {
            if !tracked {
                return Err("workflow lease was already tracked".into());
            }
            inputs
                .send(AgentInput::WorkflowTerminal(terminal))
                .await
                .map_err(|_| "workflow terminal input channel closed".to_string())?;
            Ok(WorkflowInputDisposition::Handled)
        })
    }
}

#[tokio::test]
async fn explicit_ephemeral_waits_for_started_workflow_terminal() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let provider = MultiCallProvider::new(vec![chunks::text_turn("workflow follow-up")]);
    let provider_calls = provider.messages_handle();
    kernel.register_provider(Arc::new(provider) as Arc<dyn Provider>);
    let (mut runner, _events, inputs) = make_idle_runner_with_tracked_kernel(Arc::new(kernel));
    runner.params.config.lifecycle = LifecycleMode::Ephemeral;

    let tracker = runner.params.workflow_lease_tracker.clone();
    let run_id = WorkflowRunId::new("wrun_explicit_ephemeral");
    let terminal = WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(&runner.params.session.id, run_id.clone(), 1),
        state: WorkflowRunState::Succeeded,
        run_goal: "finish delegated work".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "delegated result".into(),
        },
        content: "delegated result".into(),
    };
    let (terminal, acknowledgement) = WorkflowTerminalRequest::tracked(terminal);
    runner.params.workflow_input_handler = Some(Arc::new(StartsWorkflow {
        tracker: tracker.clone(),
        run_id,
        inputs: inputs.clone(),
        terminal,
    }));
    inputs
        .send(AgentInput::Message(Envelope::new(
            MessageSource::Human,
            "main",
            "delegate this",
        )))
        .await
        .unwrap();

    let output = runner.run().await.unwrap();

    assert_eq!(output.result, "workflow follow-up");
    assert_eq!(provider_calls.lock().unwrap().len(), 1);
    assert_eq!(
        acknowledgement.borrow().as_ref(),
        Some(&WorkflowTerminalDisposition::Applied)
    );
    assert!(!tracker.has_outstanding());
    assert!(matches!(
        runner.recorded_turns().last().map(|turn| &turn.trigger),
        Some(loopal_turn::TurnTrigger::WorkflowResult { .. })
    ));
}

#[tokio::test]
async fn explicit_ephemeral_waits_for_recovered_workflow_terminal() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let provider = MultiCallProvider::new(vec![chunks::text_turn("recovered follow-up")]);
    let provider_calls = provider.messages_handle();
    kernel.register_provider(Arc::new(provider) as Arc<dyn Provider>);
    let (mut runner, _events, inputs) = make_idle_runner_with_tracked_kernel(Arc::new(kernel));
    runner.params.config.lifecycle = LifecycleMode::Ephemeral;

    let run_id = WorkflowRunId::new("wrun_recovered_ephemeral");
    let tracker = Arc::new(WorkflowLeaseTracker::recovered(&[], vec![run_id.clone()]));
    runner.params.workflow_lease_tracker = tracker.clone();
    let terminal = WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(&runner.params.session.id, run_id, 2),
        state: WorkflowRunState::Succeeded,
        run_goal: "finish recovered work".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "recovered result".into(),
        },
        content: "recovered result".into(),
    };
    let (terminal, acknowledgement) = WorkflowTerminalRequest::tracked(terminal);
    inputs
        .send(AgentInput::WorkflowTerminal(terminal))
        .await
        .unwrap();

    let output = runner.run().await.unwrap();

    assert_eq!(output.result, "recovered follow-up");
    assert_eq!(provider_calls.lock().unwrap().len(), 1);
    assert_eq!(
        acknowledgement.borrow().as_ref(),
        Some(&WorkflowTerminalDisposition::Applied)
    );
    assert!(!tracker.has_outstanding());
    assert!(matches!(
        runner.recorded_turns().last().map(|turn| &turn.trigger),
        Some(loopal_turn::TurnTrigger::WorkflowResult { .. })
    ));
}
