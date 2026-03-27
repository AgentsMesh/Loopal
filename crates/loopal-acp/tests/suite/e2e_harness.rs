//! ACP integration test harness — drives the adapter with an in-memory agent server.

use std::sync::Arc;
use std::time::Duration;

use loopal_acp::jsonrpc::JsonRpcTransport;
use loopal_acp::run_acp_with_transport;
use loopal_error::LoopalError;
use loopal_ipc::StdioTransport;
use loopal_ipc::transport::Transport;
use loopal_provider_api::StreamChunk;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

/// ACP integration test harness with in-memory I/O.
pub struct AcpTestHarness {
    pub client_writer: DuplexStream,
    pub client_reader: BufReader<DuplexStream>,
    #[allow(dead_code)]
    pub fixture: TestFixture,
    next_id: i64,
}

/// Build an ACP harness: spawns in-memory agent server + ACP adapter.
pub fn build_acp_harness(calls: Vec<Vec<Result<StreamChunk, LoopalError>>>) -> AcpTestHarness {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();

    // ACP side: duplex for IDE ↔ ACP adapter
    let (client_writer, acp_read) = tokio::io::duplex(8192);
    let (acp_write, client_reader) = tokio::io::duplex(8192);

    // Agent server side: duplex for ACP adapter ↔ agent server
    let (adapter_to_server, server_read) = tokio::io::duplex(8192);
    let (server_to_adapter, adapter_from_server) = tokio::io::duplex(8192);

    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let session_dir = fixture.path().join("sessions");

    // Build transports — duplex wiring:
    //   adapter writes to adapter_to_server → server reads from server_read
    //   server writes to server_to_adapter → adapter reads from adapter_from_server
    let server_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(BufReader::new(server_read)),
        Box::new(server_to_adapter),
    ));
    let adapter_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(BufReader::new(adapter_from_server)),
        Box::new(adapter_to_server),
    ));

    // Spawn agent server (test mode with mock provider)
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

    // Spawn ACP adapter
    let acp_out = Arc::new(JsonRpcTransport::with_writer(Box::new(acp_write)));
    tokio::spawn(async move {
        let mut reader = BufReader::new(acp_read);
        let _ = run_acp_with_transport(adapter_transport, acp_out, &mut reader).await;
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
    /// Send a JSON-RPC request and wait for the matching response.
    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let (resp, _) = self.request_with_notifications(method, params).await;
        resp
    }

    /// Send a request and return (response, collected_notifications).
    pub async fn request_with_notifications(
        &mut self,
        method: &str,
        params: Value,
    ) -> (Value, Vec<Value>) {
        let id = self.next_id;
        self.next_id += 1;

        let msg = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params,
        });
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
