//! Tests for HubFrontend::drain_pending() — message and control routing.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_protocol::{ControlCommand, Envelope, InterruptSignal, MessageSource};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::traits::AgentFrontend;

use loopal_agent_server::hub_frontend::HubFrontend;
use loopal_agent_server::session_hub::SharedSession;

fn make_session() -> (
    Arc<SharedSession>,
    tokio::sync::mpsc::Sender<AgentInput>,
    tokio::sync::mpsc::Receiver<AgentInput>,
    tokio::sync::watch::Receiver<u64>,
) {
    let (input_tx, input_rx) = tokio::sync::mpsc::channel(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
    let session = Arc::new(SharedSession {
        session_id: "test-session".into(),
        clients: Mutex::new(Vec::new()),
        input_tx: input_tx.clone(),
        interrupt,
        interrupt_tx: Arc::new(watch_tx),
        agent_shared: Mutex::new(None),
    });
    (session, input_tx, input_rx, watch_rx)
}

#[tokio::test]
async fn test_hub_drain_pending_messages() {
    let (session, input_tx, input_rx, watch_rx) = make_session();
    let frontend = HubFrontend::new(session, input_rx, None, watch_rx);

    let env = Envelope::new(MessageSource::Human, "main", "hello");
    input_tx.send(AgentInput::Message(env)).await.unwrap();

    let pending = frontend.drain_pending().await;
    assert_eq!(pending.len(), 1);
    let AgentInput::Message(ref env) = pending[0] else {
        panic!("expected AgentInput::Message");
    };
    assert_eq!(env.content.text, "hello");
}

#[tokio::test]
async fn test_hub_drain_pending_controls() {
    let (session, input_tx, input_rx, watch_rx) = make_session();
    let frontend = HubFrontend::new(session, input_rx, None, watch_rx);

    input_tx
        .send(AgentInput::Control(ControlCommand::Clear))
        .await
        .unwrap();

    let pending = frontend.drain_pending().await;
    assert_eq!(pending.len(), 1);
    assert!(matches!(
        pending[0],
        AgentInput::Control(ControlCommand::Clear)
    ));
}
