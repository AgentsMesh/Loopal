use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;

async fn server() -> (
    Arc<Connection<loopal_ipc::connection::Listening>>,
    TestFixture,
) {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let provider =
        Arc::new(MultiCallProvider::new(Vec::new())) as Arc<dyn loopal_provider_api::Provider>;
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let server_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let client_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_transport, provider, cwd, session_dir)
                .await;
    });
    let (client, mut incoming) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while incoming.recv().await.is_some() {} });
    client
        .send_request("initialize", serde_json::json!({"protocol_version": 1}))
        .await
        .unwrap();
    (client, fixture)
}

#[tokio::test]
async fn explicit_session_id_is_preserved_by_successful_start() {
    let (client, fixture) = server().await;
    let id = uuid::Uuid::new_v4();
    let response = client
        .send_request(
            methods::AGENT_START.name,
            serde_json::json!({
                "cwd": fixture.path().to_string_lossy(),
                "session_id": id.to_string()
            }),
        )
        .await
        .unwrap();

    assert_eq!(response["session_id"], id.to_string());
    client
        .send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({}))
        .await
        .unwrap();
}
