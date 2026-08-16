use loopal_protocol::{AgentStatus, MessageSource};
use tokio::sync::mpsc;

use super::super::{InputProgress, PendingInput, SelectResult, WaitResult};
use super::support::{envelope, make_fixture, terminal};
use crate::agent_input::{AgentInput, WorkflowTerminalRequest};

#[tokio::test]
async fn pending_and_selected_input_cover_defer_queue_and_terminal_paths() {
    let mut fixture = make_fixture();
    fixture.runner.interrupt.signal();
    fixture
        .input_tx
        .send(AgentInput::Message(envelope(MessageSource::Human, "wake")))
        .await
        .unwrap();
    assert!(matches!(
        fixture.runner.wait_for_input().await.unwrap(),
        Some(WaitResult::MessageAdded)
    ));
    assert!(!fixture.runner.interrupt.is_signaled());

    let mut fixture = make_fixture();
    let (trigger_tx, trigger_rx) = mpsc::channel(1);
    fixture.runner.trigger_rx = Some(trigger_rx);
    trigger_tx
        .send(envelope(MessageSource::Scheduled, "queued"))
        .await
        .unwrap();
    let queued = fixture.runner.poll_pending_input().await.unwrap();
    let PendingInput::Queued(queued) = queued else {
        panic!("scheduled input was not queued");
    };
    assert!(matches!(
        fixture.runner.consume_queued_input(queued).await,
        WaitResult::MessageAdded
    ));

    let mut fixture = make_fixture();
    fixture.runner.status = AgentStatus::Suspended;
    fixture
        .input_tx
        .send(AgentInput::Message(envelope(
            MessageSource::Agent("peer".into()),
            "deferred while suspended",
        )))
        .await
        .unwrap();
    assert!(matches!(
        fixture.runner.poll_pending_input().await.unwrap(),
        PendingInput::Empty
    ));
    assert_eq!(fixture.runner.deferred_frontend_inputs.len(), 1);

    let mut fixture = make_fixture();
    assert!(matches!(
        fixture
            .runner
            .process_selected_input(SelectResult::ChannelClosed)
            .await
            .unwrap(),
        InputProgress::Continue
    ));
    let notification = terminal(&fixture.runner.params.session.id, "wrun_input", 1);
    let (request, _ack) = WorkflowTerminalRequest::tracked(notification.clone());
    assert!(matches!(
        fixture
            .runner
            .process_selected_input(SelectResult::AgentInput(Some(
                AgentInput::WorkflowTerminal(request)
            )))
            .await
            .unwrap(),
        InputProgress::Ready(WaitResult::WorkflowResultAdded)
    ));
    let (duplicate, _ack) = WorkflowTerminalRequest::tracked(notification);
    assert!(matches!(
        fixture
            .runner
            .process_selected_input(SelectResult::AgentInput(Some(
                AgentInput::WorkflowTerminal(duplicate)
            )))
            .await
            .unwrap(),
        InputProgress::Continue
    ));

    let mut fixture = make_fixture();
    fixture.runner.status = AgentStatus::Suspended;
    let notification = terminal(&fixture.runner.params.session.id, "wrun_deferred", 2);
    let (request, _ack) = WorkflowTerminalRequest::tracked(notification);
    assert!(matches!(
        fixture
            .runner
            .process_selected_input(SelectResult::AgentInput(Some(
                AgentInput::WorkflowTerminal(request)
            )))
            .await
            .unwrap(),
        InputProgress::Continue
    ));
}
