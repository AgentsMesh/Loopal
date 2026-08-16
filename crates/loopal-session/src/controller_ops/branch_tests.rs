use std::sync::Arc;

use loopal_agent_hub::LocalChannels;
use loopal_protocol::{InterruptSignal, UserContent};
use tokio::sync::{mpsc, watch};

use super::ControlBackend;

fn local_backend(mailbox_tx: mpsc::Sender<loopal_protocol::Envelope>) -> ControlBackend {
    let (control_tx, _) = mpsc::channel(1);
    let (permission_tx, _) = mpsc::channel(1);
    let (question_tx, _) = mpsc::channel(1);
    let (interrupt_tx, _) = watch::channel(0);
    ControlBackend::Local(Arc::new(LocalChannels {
        control_tx,
        permission_tx,
        question_tx,
        mailbox_tx: Some(mailbox_tx),
        interrupt: InterruptSignal::new(),
        interrupt_tx: Arc::new(interrupt_tx),
    }))
}

#[tokio::test]
async fn local_route_delivers_to_configured_mailbox() {
    let (mailbox_tx, mut mailbox_rx) = mpsc::channel(1);
    let backend = local_backend(mailbox_tx);

    backend
        .route_to_agent("child", UserContent::text_only("hello"))
        .await;

    let envelope = mailbox_rx.recv().await.unwrap();
    assert_eq!(envelope.target.agent, "child");
    assert_eq!(envelope.content.text, "hello");
}

#[tokio::test]
async fn local_route_swallows_closed_mailbox_error() {
    let (mailbox_tx, mailbox_rx) = mpsc::channel(1);
    drop(mailbox_rx);
    local_backend(mailbox_tx)
        .route_to_agent("child", UserContent::text_only("dropped"))
        .await;
}
