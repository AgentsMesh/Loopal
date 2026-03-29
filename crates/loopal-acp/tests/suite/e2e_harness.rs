//! ACP integration test harness — real Hub + mock agent server.
//!
//! Tests exercise the same path as production: ACP → UiSession → Hub → Agent.

use std::sync::Arc;
use std::time::Duration;

use loopal_acp::AcpAdapter;
use loopal_acp::jsonrpc::JsonRpcTransport;
use loopal_agent_hub::Hub;
use loopal_agent_hub::UiSession;
use loopal_error::LoopalError;
use loopal_ipc::StdioTransport;
use loopal_ipc::connection::Connection;
use loopal_ipc::transport::Transport;
use loopal_provider_api::StreamChunk;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::Mutex;

pub struct AcpTestHarness {
    pub client_writer: DuplexStream,
    pub client_reader: BufReader<DuplexStream>,
    #[allow(dead_code)]
    pub fixture: TestFixture,
    next_id: i64,
}

/// Build a Hub-backed ACP harness with mock agent server.
pub async fn build_acp_harness(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
) -> AcpTestHarness {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();

    // IDE ↔ ACP adapter
    let (client_writer, acp_read) = tokio::io::duplex(8192);
    let (acp_write, client_reader) = tokio::io::duplex(8192);

    // Hub ↔ Agent server
    let (hub_to_server, server_read) = tokio::io::duplex(8192);
    let (server_to_hub, hub_from_server) = tokio::io::duplex(8192);

    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let session_dir = fixture.path().join("sessions");

    let server_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(BufReader::new(server_read)),
        Box::new(server_to_hub),
    ));
    let agent_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(BufReader::new(hub_from_server)),
        Box::new(hub_to_server),
    ));

    // 1. Spawn mock agent server
    tokio::spawn({
        let cwd = cwd.clone();
        async move {
            let _ = loopal_agent_server::run_server_for_test_interactive(
                server_transport,
                provider,
                cwd,
                session_dir,
            )
            .await;
        }
    });

    // 2. Create Hub
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(256);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));

    // 3. Start event broadcast EARLY (before agent starts)
    let _event_loop = loopal_agent_hub::start_event_loop(hub.clone(), event_rx);

    // 4. Connect ACP as UI client via UiSession BEFORE agent starts
    let ui_session = UiSession::connect(hub.clone(), "acp").await;

    // 5. Connect to agent, initialize + start it
    let agent_conn = Arc::new(Connection::new(agent_transport));
    let agent_incoming = agent_conn.start();
    let _ = agent_conn
        .send_request("initialize", serde_json::json!({"protocol_version": 1}))
        .await;
    let _ = agent_conn
        .send_request("agent/start", serde_json::json!({"cwd": cwd}))
        .await;

    // 6. Register agent in Hub and spawn IO loop
    {
        let mut h = hub.lock().await;
        let _ = h.registry.register_connection("main", agent_conn.clone());
    }
    loopal_agent_hub::agent_io::spawn_io_loop(
        hub.clone(),
        "main",
        agent_conn,
        agent_incoming,
        true,
    );

    // 7. Spawn ACP adapter using UiSession
    let acp_out = Arc::new(JsonRpcTransport::with_writer(Box::new(acp_write)));
    tokio::spawn(async move {
        let adapter = AcpAdapter::new(ui_session, acp_out);
        let mut reader = BufReader::new(acp_read);
        let _ = adapter.run(&mut reader).await;
    });

    AcpTestHarness {
        client_writer,
        client_reader: BufReader::new(client_reader),
        fixture,
        next_id: 1,
    }
}

const IO_TIMEOUT: Duration = Duration::from_secs(10);

impl AcpTestHarness {
    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let (resp, _) = self.request_with_notifications(method, params).await;
        resp
    }

    pub async fn send_notification(&mut self, method: &str, params: Value) {
        let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        let mut bytes = serde_json::to_vec(&msg).unwrap();
        bytes.push(b'\n');
        self.client_writer.write_all(&bytes).await.unwrap();
        self.client_writer.flush().await.unwrap();
    }

    pub async fn request_with_notifications(
        &mut self,
        method: &str,
        params: Value,
    ) -> (Value, Vec<Value>) {
        let id = self.next_id;
        self.next_id += 1;
        let msg =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let mut bytes = serde_json::to_vec(&msg).unwrap();
        bytes.push(b'\n');
        self.client_writer.write_all(&bytes).await.unwrap();
        self.client_writer.flush().await.unwrap();

        let mut notifications = Vec::new();
        loop {
            let mut line = String::new();
            match tokio::time::timeout(IO_TIMEOUT, self.client_reader.read_line(&mut line)).await {
                Ok(Ok(_)) => {
                    let parsed: Value = serde_json::from_str(line.trim()).unwrap();
                    if parsed.get("id").and_then(|v| v.as_i64()) == Some(id) {
                        return (parsed, notifications);
                    }
                    notifications.push(parsed);
                }
                _ => panic!("timeout waiting for JSON-RPC response to {method}"),
            }
        }
    }
}
