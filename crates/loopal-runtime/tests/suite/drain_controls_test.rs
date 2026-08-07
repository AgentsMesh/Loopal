//! Tests for UnifiedFrontend::drain_pending() — control commands and mixed input.

use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentMode, Envelope, MessageSource};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::UnifiedFrontend;
use loopal_runtime::frontend::{AgentFrontend, DenyAllHandler, UnsupportedQuestionHandler};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
        Box::new(DenyAllHandler),
        Box::new(UnsupportedQuestionHandler),
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

#[tokio::test]
async fn try_recv_prioritizes_queued_control_over_mailbox() {
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);
    let f = make_unified(mb_rx, ctrl_rx);

    mb_tx
        .send(Envelope::new(MessageSource::Human, "main", "queued input"))
        .await
        .unwrap();
    ctrl_tx.send(ControlCommand::Suspend).await.unwrap();

    assert!(matches!(
        f.try_recv_input().await.unwrap(),
        AgentInput::Control(ControlCommand::Suspend)
    ));
    assert!(matches!(
        f.try_recv_input().await.unwrap(),
        AgentInput::Message(_)
    ));
    assert!(matches!(
        f.try_recv_input().await,
        Err(mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn try_recv_keeps_live_mailbox_after_control_channel_closes() {
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);
    let f = make_unified(mb_rx, ctrl_rx);

    mb_tx
        .send(Envelope::new(MessageSource::Human, "main", "last input"))
        .await
        .unwrap();
    drop(ctrl_tx);

    assert!(matches!(
        f.try_recv_input().await.unwrap(),
        AgentInput::Message(_)
    ));
    assert!(matches!(
        f.try_recv_input().await,
        Err(mpsc::error::TryRecvError::Empty)
    ));
    mb_tx
        .send(Envelope::new(MessageSource::Human, "main", "still live"))
        .await
        .unwrap();
    assert!(matches!(
        f.try_recv_input().await.unwrap(),
        AgentInput::Message(_)
    ));
    drop(mb_tx);
    assert!(matches!(
        f.try_recv_input().await,
        Err(mpsc::error::TryRecvError::Disconnected)
    ));
}

#[tokio::test]
async fn recv_input_waits_on_mailbox_after_control_channel_closes() {
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);
    let f = std::sync::Arc::new(make_unified(mb_rx, ctrl_rx));
    drop(ctrl_tx);

    let waiting = {
        let f = f.clone();
        tokio::spawn(async move { f.recv_input().await })
    };
    tokio::task::yield_now().await;
    assert!(
        !waiting.is_finished(),
        "one closed plane ended the frontend"
    );
    mb_tx
        .send(Envelope::new(MessageSource::Human, "main", "arrived later"))
        .await
        .unwrap();
    assert!(matches!(
        waiting.await.unwrap(),
        Some(AgentInput::Message(_))
    ));
}

#[tokio::test]
async fn recv_input_waits_on_control_after_mailbox_channel_closes() {
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);
    let f = std::sync::Arc::new(make_unified(mb_rx, ctrl_rx));
    drop(mb_tx);

    let waiting = {
        let f = f.clone();
        tokio::spawn(async move { f.recv_input().await })
    };
    tokio::task::yield_now().await;
    assert!(
        !waiting.is_finished(),
        "one closed plane ended the frontend"
    );
    ctrl_tx.send(ControlCommand::Suspend).await.unwrap();
    assert!(matches!(
        waiting.await.unwrap(),
        Some(AgentInput::Control(ControlCommand::Suspend))
    ));
}

#[tokio::test]
async fn try_recv_reports_cancel_before_queued_work() {
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let f = UnifiedFrontend::new(
        Some("sub".into()),
        event_tx,
        mb_rx,
        ctrl_rx,
        Some(cancel.clone()),
        Box::new(DenyAllHandler),
        Box::new(UnsupportedQuestionHandler),
    );
    mb_tx
        .send(Envelope::new(MessageSource::Human, "main", "must not run"))
        .await
        .unwrap();
    cancel.cancel();

    assert!(matches!(
        f.try_recv_input().await,
        Err(mpsc::error::TryRecvError::Disconnected)
    ));
}
