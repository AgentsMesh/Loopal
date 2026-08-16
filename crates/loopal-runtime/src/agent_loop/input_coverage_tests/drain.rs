use loopal_protocol::{ControlCommand, MessageSource};
use tokio::sync::mpsc;

use super::super::WaitResult;
use super::support::{FAIL_PERMISSION, envelope, make_fixture, terminal};
use crate::agent_input::{
    AgentInput, ControlAcknowledgement, ControlRequest, WorkflowTerminalRequest,
};

#[tokio::test]
async fn drain_pending_applies_controls_terminals_and_automatic_envelopes() {
    let mut fixture = make_fixture();
    fixture
        .runner
        .deferred_frontend_inputs
        .push_back(AgentInput::Message(envelope(
            MessageSource::Agent("deferred".into()),
            "deferred",
        )));
    let (tracked, mut tracked_ack) =
        ControlRequest::tracked(ControlCommand::DecisionModeSwitch("agent".into()));
    let notification = terminal(&fixture.runner.params.session.id, "wrun_drain", 1);
    let (terminal_request, _terminal_ack) = WorkflowTerminalRequest::tracked(notification);
    fixture.frontend.set_drained(vec![
        AgentInput::Message(envelope(MessageSource::Human, "frontend")),
        AgentInput::Control(ControlCommand::ModelSwitch(" ".into())),
        AgentInput::TrackedControl(tracked),
        AgentInput::WorkflowTerminal(terminal_request),
    ]);
    let (trigger_tx, trigger_rx) = mpsc::channel(1);
    let (rewake_tx, rewake_rx) = mpsc::channel(1);
    fixture.runner.trigger_rx = Some(trigger_rx);
    fixture.runner.rewake_rx = Some(rewake_rx);
    trigger_tx
        .send(envelope(MessageSource::Scheduled, "trigger"))
        .await
        .unwrap();
    rewake_tx
        .send(envelope(MessageSource::Scheduled, "rewake"))
        .await
        .unwrap();

    let pending = fixture.runner.drain_pending_input().await;
    assert_eq!(
        pending
            .iter()
            .map(|value| value.content.text.as_str())
            .collect::<Vec<_>>(),
        ["deferred", "frontend", "trigger", "rewake"]
    );
    assert!(matches!(
        tracked_ack.recv().await,
        Some(ControlAcknowledgement::Rejected(reason)) if reason.contains("not implemented")
    ));

    let mut fixture = make_fixture();
    let notification = terminal(&fixture.runner.params.session.id, "wrun_consume", 1);
    let (request, _ack) = WorkflowTerminalRequest::tracked(notification.clone());
    assert!(matches!(
        fixture
            .runner
            .consume_frontend_data(AgentInput::WorkflowTerminal(request))
            .await,
        WaitResult::WorkflowResultAdded
    ));
    let (duplicate, _ack) = WorkflowTerminalRequest::tracked(notification);
    assert!(matches!(
        fixture
            .runner
            .consume_frontend_data(AgentInput::WorkflowTerminal(duplicate))
            .await,
        WaitResult::WorkflowHandled
    ));
}

#[tokio::test]
async fn drain_continues_after_untracked_and_tracked_control_emit_failures() {
    let mut fixture = make_fixture();
    fixture.frontend.set_fail_mask(FAIL_PERMISSION);
    let (tracked, mut acknowledgement) =
        ControlRequest::tracked(ControlCommand::PermissionModeSwitch("ask_any_write".into()));
    fixture.frontend.set_drained(vec![
        AgentInput::Control(ControlCommand::PermissionModeSwitch("bypass".into())),
        AgentInput::TrackedControl(tracked),
    ]);

    assert!(fixture.runner.drain_pending_input().await.is_empty());
    assert!(matches!(
        acknowledgement.recv().await,
        Some(ControlAcknowledgement::Rejected(reason))
            if reason.contains("runtime failed to apply control")
    ));
}
