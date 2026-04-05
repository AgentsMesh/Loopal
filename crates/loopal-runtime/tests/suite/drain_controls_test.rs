//! Tests for UnifiedFrontend::drain_pending() — control commands and mixed input.

use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentMode, Envelope, MessageSource};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::UnifiedFrontend;
use loopal_runtime::frontend::{AgentFrontend, AutoCancelQuestionHandler, AutoDenyHandler};
use tokio::sync::mpsc;

fn make_unified(
    mailbox_rx: mpsc::Receiver<Envelope>,
    control_rx: mpsc::Receiver<ControlCommand>,
) -> UnifiedFrontend {
    let (event_tx, _event_rx) = mpsc::channel(16);
    UnifiedFrontend::new(
        Some("sub".into()),
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    )
}

#[tokio::test]
async fn test_unified_drain_pending_with_controls() {
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(mb_rx, ctrl_rx);
    ctrl_tx
        .send(ControlCommand::ModeSwitch(AgentMode::Plan))
        .await
        .unwrap();

    let pending = f.drain_pending().await;
    assert_eq!(pending.len(), 1);
    assert!(matches!(
        pending[0],
        AgentInput::Control(ControlCommand::ModeSwitch(AgentMode::Plan))
    ));
}

#[tokio::test]
async fn test_unified_drain_pending_mixed() {
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(mb_rx, ctrl_rx);

    mb_tx
        .send(Envelope::new(
            MessageSource::Agent("lead".into()),
            "sub",
            "do task",
        ))
        .await
        .unwrap();
    ctrl_tx.send(ControlCommand::Clear).await.unwrap();

    let pending = f.drain_pending().await;
    assert_eq!(pending.len(), 2);
    // Messages come first (mailbox drained before control channel)
    assert!(matches!(pending[0], AgentInput::Message(_)));
    assert!(matches!(
        pending[1],
        AgentInput::Control(ControlCommand::Clear)
    ));
}
