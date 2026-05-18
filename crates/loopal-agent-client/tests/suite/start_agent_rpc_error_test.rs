use std::sync::Arc;
use std::time::Duration;

use loopal_agent_client::{AgentClient, StartAgentParams};
use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;

fn make_pair() -> (Arc<dyn loopal_ipc::transport::Transport>, Arc<Connection>) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let client_transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(
        StdioTransport::new(Box::new(tokio::io::BufReader::new(b_rx)), Box::new(a_tx)),
    );
    let server_transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(
        StdioTransport::new(Box::new(tokio::io::BufReader::new(a_rx)), Box::new(b_tx)),
    );
    let server_conn = Arc::new(Connection::new(server_transport));
    (client_transport, server_conn)
}

#[tokio::test]
async fn start_agent_returns_err_on_rpc_error_response() {
    let (transport, server) = make_pair();
    let mut server_rx = server.start();
    let client = AgentClient::new(transport);

    let server_clone = server.clone();
    tokio::spawn(async move {
        while let Some(msg) = server_rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = if method == methods::AGENT_START.name {
                    server_clone
                        .respond_error(id, -32603, "session not found: bogus-id")
                        .await
                } else {
                    server_clone
                        .respond(id, serde_json::json!({"protocol_version": 1}))
                        .await
                };
            }
        }
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        client.start_agent(&StartAgentParams {
            cwd: std::env::current_dir().unwrap(),
            resume: Some("bogus-id".into()),
            ..Default::default()
        }),
    )
    .await
    .expect("start_agent must not exceed 25s timeout when server responds quickly");

    let err = outcome.expect_err("rpc error must propagate as Err");
    let msg = err.to_string();
    assert!(
        msg.contains("session not found"),
        "Err should surface server-provided detail, got: {msg}"
    );
}
