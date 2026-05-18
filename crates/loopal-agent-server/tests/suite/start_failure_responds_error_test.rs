use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;
use loopal_ipc::rpc_error::RpcError;
use loopal_ipc::transport::Transport;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;

fn pair_with_server(
    cwd: std::path::PathBuf,
    session_dir: std::path::PathBuf,
) -> (Arc<Connection>, tokio::sync::mpsc::Receiver<Incoming>) {
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
    let client = Arc::new(Connection::new(client_transport));
    let rx = client.start();
    (client, rx)
}

#[tokio::test]
async fn resume_unknown_session_returns_rpc_error_not_hang() {
    let fixture = TestFixture::new();
    let (client, _rx) = pair_with_server(
        fixture.path().to_path_buf(),
        fixture.path().join("sessions"),
    );

    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .expect("initialize completes within 5s")
    .expect("initialize ok");

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        client.send_request(
            methods::AGENT_START.name,
            serde_json::json!({"resume": "nonexistent-session-id"}),
        ),
    )
    .await
    .expect("agent/start must respond within 5s, not hang");

    let err = outcome.expect_err("rpc error must surface as Err");
    match &err {
        RpcError::Remote { code, message, .. } => {
            assert_eq!(*code, jsonrpc::INTERNAL_ERROR);
            assert!(
                message.contains("nonexistent-session-id"),
                "msg must name the bad id: {message}"
            );
        }
        _ => panic!("expected Remote, got: {err:?}"),
    }
}
