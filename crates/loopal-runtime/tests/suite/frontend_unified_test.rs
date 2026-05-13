use loopal_protocol::AgentMode;
use loopal_protocol::ControlCommand;
use loopal_protocol::{Envelope, MessageSource};
use loopal_runtime::agent_input::AgentInput;
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

// --- recv_input from envelope ---

#[tokio::test]
async fn test_unified_recv_input_from_envelope() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        None,
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );

    let env = Envelope::new(MessageSource::Human, "main", "hello");
    mb_tx.send(env).await.unwrap();

    let cmd = f.recv_input().await;
    match cmd {
        Some(AgentInput::Message(env)) => {
            assert!(matches!(env.source, MessageSource::Human));
            assert_eq!(env.content.text, "hello");
        }
        other => panic!("expected AgentInput::Message, got {other:?}"),
    }
}

// --- recv_input from control ---

#[tokio::test]
async fn test_unified_recv_input_from_control_mode_switch() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        None,
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    ctrl_tx
        .send(ControlCommand::ModeSwitch(AgentMode::Plan))
        .await
        .unwrap();

    let cmd = f.recv_input().await;
    assert!(matches!(
        cmd,
        Some(AgentInput::Control(ControlCommand::ModeSwitch(
            AgentMode::Plan
        )))
    ));
}

#[tokio::test]
async fn test_unified_recv_input_from_control_clear() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        None,
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    ctrl_tx.send(ControlCommand::Clear).await.unwrap();

    assert!(matches!(
        f.recv_input().await,
        Some(AgentInput::Control(ControlCommand::Clear))
    ));
}

// --- shutdown (via channel close) ---

#[tokio::test]
async fn test_unified_shutdown_returns_none() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        None,
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );
    drop(mb_tx);
    drop(ctrl_tx);

    assert!(f.recv_input().await.is_none());
}

// --- cancel token ---

#[tokio::test]
async fn test_unified_cancel_token_breaks_recv() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);
    let token = CancellationToken::new();

    let f = make_unified(
        None,
        event_tx,
        mb_rx,
        ctrl_rx,
        Some(token.clone()),
        Box::new(DenyAllHandler),
    );
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        token.cancel();
    });

    assert!(f.recv_input().await.is_none());
}

// --- drain_pending ---

#[tokio::test]
async fn test_unified_drain_pending() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        Some("sub".into()),
        event_tx,
        mb_rx,
        ctrl_rx,
        None,
        Box::new(DenyAllHandler),
    );

    let e1 = Envelope::new(MessageSource::Agent("lead".into()), "sub", "task A");
    let e2 = Envelope::new(MessageSource::Agent("peer".into()), "sub", "task B");
    mb_tx.send(e1).await.unwrap();
    mb_tx.send(e2).await.unwrap();

    let pending = f.drain_pending().await;
    assert_eq!(pending.len(), 2);
    // drain_pending returns AgentInput; extract Envelope via pattern match.
    let AgentInput::Message(ref env0) = pending[0] else {
        panic!("expected Message");
    };
    let AgentInput::Message(ref env1) = pending[1] else {
        panic!("expected Message");
    };
    assert_eq!(env0.source.label(), "lead");
    assert_eq!(env0.content.text, "task A");
    assert_eq!(env1.source.label(), "peer");
    assert_eq!(env1.content.text, "task B");
}
