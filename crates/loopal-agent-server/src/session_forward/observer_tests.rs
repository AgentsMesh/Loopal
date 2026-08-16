use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, InterruptSignal, MessageSource};
use loopal_runtime::agent_input::AgentInput;

use super::forwarding_test_support::peers;
use crate::session_hub::SharedSession;

#[tokio::test]
async fn routes_control_message_and_interrupt_then_removes_client() {
    let ((server, mut server_rx), (client, _client_rx)) = peers();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel::<AgentInput>(2);
    let interrupt = InterruptSignal::new();
    let (interrupt_tx, mut interrupt_rx) = tokio::sync::watch::channel(0u64);
    let session = Arc::new(SharedSession::new(
        "observer-routing-session".into(),
        input_tx,
        interrupt.clone(),
        Arc::new(interrupt_tx),
    ));
    session.add_client("observer".into(), server.clone()).await;

    let ((), ()) = tokio::join!(
        super::observer_loop(&mut server_rx, &server, &session, "observer"),
        async {
            let error = client
                .send_request(
                    methods::AGENT_CONTROL.name,
                    serde_json::json!({"invalid": true}),
                )
                .await
                .unwrap_err();
            assert_eq!(
                error.remote_code(),
                Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
            );
            assert_eq!(
                client
                    .send_request(
                        methods::AGENT_MESSAGE.name,
                        serde_json::to_value(Envelope::new(
                            MessageSource::Human,
                            "main",
                            "observer message",
                        ))
                        .unwrap(),
                    )
                    .await
                    .unwrap()["ok"],
                true
            );
            client
                .send_notification(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
                .await
                .unwrap();
            client
                .send_notification("agent/observer-ignored", serde_json::Value::Null)
                .await
                .unwrap();
            client.close().await;
        }
    );

    assert!(session.clients.lock().await.is_empty());
    assert!(interrupt.is_signaled());
    interrupt_rx.changed().await.unwrap();
    assert!(matches!(
        input_rx.recv().await,
        Some(AgentInput::Message(_))
    ));
}
