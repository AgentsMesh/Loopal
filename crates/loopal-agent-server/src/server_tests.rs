use loopal_ipc::connection::{Connection, Incoming};

use super::*;

#[tokio::test]
async fn production_connection_initializes_ignores_notifications_and_shuts_down() {
    let (server_transport, client_transport) = loopal_ipc::duplex_pair();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();

    let (server_result, ()) =
        tokio::join!(run_agent_server_on_transport(server_transport), async {
            let initialized = client
                .send_request(
                    methods::INITIALIZE.name,
                    serde_json::json!({"protocol_version": 1}),
                )
                .await
                .unwrap();
            assert_eq!(initialized["protocol_version"], 1);

            client
                .send_notification("agent/ignored-test-notification", serde_json::Value::Null)
                .await
                .unwrap();
            assert_eq!(
                client
                    .send_request(methods::AGENT_SHUTDOWN.name, serde_json::Value::Null)
                    .await
                    .unwrap()["ok"],
                true
            );

            let Incoming::Notification { method, params } = client_rx.recv().await.unwrap() else {
                panic!("expected agent completion notification")
            };
            assert_eq!(method, methods::AGENT_COMPLETED.name);
            let completion: loopal_protocol::AgentCompletion =
                serde_json::from_value(params).unwrap();
            assert_eq!(completion.reason, "shutdown");
        });
    server_result.unwrap();
}

#[tokio::test]
async fn injected_mock_provider_uses_the_same_initialized_shutdown_state_machine() {
    let fixture = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(fixture.path(), "[]").unwrap();
    let provider =
        crate::mock_loader::load_mock_provider(fixture.path().to_str().unwrap()).unwrap();
    let (server_transport, client_transport) = loopal_ipc::duplex_pair();
    let (client, mut client_rx) = Connection::new(client_transport).into_listening();

    let (server_result, ()) = tokio::join!(
        run_agent_server_with_mock_provider(provider, server_transport),
        async {
            assert_eq!(
                client
                    .send_request(
                        methods::INITIALIZE.name,
                        serde_json::json!({"protocol_version": 1}),
                    )
                    .await
                    .unwrap()["protocol_version"],
                1
            );
            assert_eq!(
                client
                    .send_request(methods::AGENT_SHUTDOWN.name, serde_json::Value::Null)
                    .await
                    .unwrap()["ok"],
                true
            );
            let Incoming::Notification { method, .. } = client_rx.recv().await.unwrap() else {
                panic!("expected agent completion notification")
            };
            assert_eq!(method, methods::AGENT_COMPLETED.name);
        }
    );
    server_result.unwrap();
}

#[tokio::test]
async fn mock_entrypoint_rejects_missing_fixture_before_opening_stdio() {
    let missing =
        std::env::temp_dir().join(format!("loopal-missing-mock-{}.json", uuid::Uuid::new_v4()));
    let error = run_agent_server_with_mock(missing.to_str().unwrap())
        .await
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::NotFound)
    );
}
