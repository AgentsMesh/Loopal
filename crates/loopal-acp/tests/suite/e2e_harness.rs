//! ACP integration test harness — drives the JSON-RPC server with in-memory I/O.

use std::sync::Arc;
use std::time::Duration;

use loopal_acp::jsonrpc::JsonRpcTransport;
use loopal_acp::{AcpHandler, run_acp_loop};
use loopal_config::{ResolvedConfig, Settings};
use loopal_error::LoopalError;
use loopal_provider_api::{Provider, StreamChunk};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

/// ACP integration test harness with in-memory stdin/stdout simulation.
pub struct AcpTestHarness {
    pub client_writer: DuplexStream,
    pub client_reader: BufReader<DuplexStream>,
    #[allow(dead_code)]
    pub fixture: TestFixture,
    next_id: i64,
}

/// Build an ACP harness with mock provider and in-memory I/O pipes.
pub fn build_acp_harness(calls: Vec<Vec<Result<StreamChunk, LoopalError>>>) -> AcpTestHarness {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();

    let (client_writer, server_read) = tokio::io::duplex(8192);
    let (server_write, client_reader) = tokio::io::duplex(8192);

    let transport = Arc::new(JsonRpcTransport::with_writer(Box::new(server_write)));
    let provider = Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>;

    let config = ResolvedConfig {
        settings: Settings::default(),
        mcp_servers: Default::default(),
        skills: Default::default(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        layers: Vec::new(),
    };

    let session_dir = fixture.path().join("sessions");
    let handler = Arc::new(AcpHandler::with_test_overrides(
        transport.clone(),
        config,
        cwd,
        provider,
        session_dir,
    ));

    let mut reader = BufReader::new(server_read);
    tokio::spawn(async move {
        let _ = run_acp_loop(&handler, &transport, &mut reader).await;
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
    /// Intermediate notifications are silently skipped.
    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let (resp, _notifications) = self.request_with_notifications(method, params).await;
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
