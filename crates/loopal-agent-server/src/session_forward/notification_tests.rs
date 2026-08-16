use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, InterruptSignal, MessageSource};
use loopal_runtime::agent_input::AgentInput;

use super::forwarding_test_support::{peers, pending_handle, session};
use super::{ForwardResult, forward_loop};
use crate::session_hub::SharedSession;

#[tokio::test]
async fn routes_interrupt_and_valid_message_then_ignores_malformed_message() {
    let ((server, mut server_rx), (client, _client_rx)) = peers();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel::<AgentInput>(4);
    let interrupt = InterruptSignal::new();
    let (interrupt_tx, mut interrupt_rx) = tokio::sync::watch::channel(0u64);
    let session = Arc::new(SharedSession::new(
        "notification-session".into(),
        input_tx,
        interrupt.clone(),
        Arc::new(interrupt_tx),
    ));
    let mut handle = pending_handle(session);
    let envelope = Envelope::new(MessageSource::Human, "main", "forward me");

    let (outcome, ()) = tokio::join!(forward_loop(&mut server_rx, &server, &mut handle), async {
        client
            .send_notification(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
            .await
            .unwrap();
        client
            .send_notification(
                methods::AGENT_MESSAGE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .unwrap();
        client
            .send_notification(
                methods::AGENT_MESSAGE.name,
                serde_json::json!({"not": "an envelope"}),
            )
            .await
            .unwrap();
        client
            .send_notification("agent/ignored-notification", serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(
            client
                .send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({}))
                .await
                .unwrap()["ok"],
            true
        );
    });

    assert!(matches!(outcome, ForwardResult::Shutdown));
    assert!(interrupt.is_signaled());
    interrupt_rx.changed().await.unwrap();
    let AgentInput::Message(received) = input_rx.recv().await.unwrap() else {
        panic!("expected forwarded message")
    };
    assert_eq!(received.content_preview(), "forward me");
    assert!(input_rx.try_recv().is_err());
    handle.agent_task.abort();
}

#[tokio::test]
async fn closed_input_drops_message_without_stalling_shutdown() {
    let ((server, mut server_rx), (client, _client_rx)) = peers();
    let (session, input_rx) = session("closed-notification-session", 1);
    drop(input_rx);
    let mut handle = pending_handle(session);

    let (outcome, ()) = tokio::join!(forward_loop(&mut server_rx, &server, &mut handle), async {
        client
            .send_notification(
                methods::AGENT_MESSAGE.name,
                serde_json::to_value(Envelope::new(MessageSource::Human, "main", "closed"))
                    .unwrap(),
            )
            .await
            .unwrap();
        client
            .send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({}))
            .await
            .unwrap();
    });
    assert!(matches!(outcome, ForwardResult::Shutdown));
    handle.agent_task.abort();
}
