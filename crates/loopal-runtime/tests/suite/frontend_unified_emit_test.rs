use loopal_protocol::AgentEventPayload;
use loopal_protocol::{ControlCommand, Envelope};
use loopal_runtime::frontend::AgentFrontend;
use loopal_runtime::frontend::{DenyAllHandler, UnifiedFrontend, UnsupportedQuestionHandler};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn make_unified(
    agent_name: Option<String>,
    event_tx: mpsc::Sender<loopal_protocol::AgentEvent>,
    mailbox_rx: mpsc::Receiver<Envelope>,
    control_rx: mpsc::Receiver<ControlCommand>,
    cancel_token: Option<CancellationToken>,
    handler: Box<dyn loopal_runtime::frontend::PermissionHandler>,
) -> UnifiedFrontend {
    UnifiedFrontend::new(
        agent_name,
        event_tx,
        mailbox_rx,
        control_rx,
        cancel_token,
        handler,
        Box::new(UnsupportedQuestionHandler),
    )
}

#[tokio::test]
async fn test_unified_emit_root_delivers_event() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        None,
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    f.emit(AgentEventPayload::Started).await.unwrap();

    let event = event_rx.recv().await.unwrap();
    assert!(event.agent_name.is_none());
    assert!(matches!(event.payload, AgentEventPayload::Started));
}

#[tokio::test]
async fn test_unified_emit_wraps_agent_name() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        Some("researcher".into()),
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    f.emit(AgentEventPayload::Finished).await.unwrap();

    let event = event_rx.recv().await.unwrap();
    assert_eq!(
        event.agent_name.as_ref().map(|a| a.to_string()).as_deref(),
        Some("researcher")
    );
}

#[tokio::test]
async fn test_unified_emit_subagent_returns_err_on_closed_channel() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);
    drop(event_rx);

    let f = make_unified(
        Some("sub".into()),
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    // emit is always-fallible — root/sub symmetry. Callers wanting silent
    // swallow opt into `emit_best_effort`.
    assert!(f.emit(AgentEventPayload::Started).await.is_err());
}

#[tokio::test]
async fn test_unified_emit_best_effort_swallows_closed_channel() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);
    drop(event_rx);

    let f = make_unified(
        Some("sub".into()),
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    f.emit_best_effort(AgentEventPayload::Started, "test").await;
}
