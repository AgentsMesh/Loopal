use std::sync::Arc;

use loopal_agent_hub::HubClient;
use loopal_session::SessionController;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

pub(super) struct HubHarness {
    pub(super) controller: SessionController,
    reader: BufReader<tokio::io::ReadHalf<DuplexStream>>,
    writer: tokio::io::WriteHalf<DuplexStream>,
}

impl HubHarness {
    pub(super) fn new() -> Self {
        let (client_end, server_end) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client_end);
        let (server_reader, server_writer) = tokio::io::split(server_end);
        let transport: Arc<dyn loopal_ipc::transport::Transport> =
            Arc::new(loopal_ipc::StdioTransport::new(
                Box::new(BufReader::new(client_reader)),
                Box::new(client_writer),
            ));
        let (connection, _incoming) =
            loopal_ipc::connection::Connection::new(transport).into_listening();
        Self {
            controller: SessionController::with_hub(Arc::new(HubClient::new(connection))),
            reader: BufReader::new(server_reader),
            writer: server_writer,
        }
    }

    pub(super) async fn read_request(&mut self) -> Value {
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            self.reader.read_line(&mut line),
        )
        .await
        .expect("request timeout")
        .expect("request read");
        serde_json::from_str(line.trim()).expect("valid JSON-RPC request")
    }

    pub(super) async fn respond_ok(&mut self, request: &Value, result: Value) {
        self.write_response(serde_json::json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": result,
        }))
        .await;
    }

    pub(super) async fn respond_error(&mut self, request: &Value, message: &str) {
        self.write_response(serde_json::json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "error": {"code": -32000, "message": message},
        }))
        .await;
    }

    async fn write_response(&mut self, response: Value) {
        let mut frame = serde_json::to_vec(&response).unwrap();
        frame.push(b'\n');
        self.writer.write_all(&frame).await.unwrap();
        self.writer.flush().await.unwrap();
    }
}

pub(super) fn disconnected_controller() -> SessionController {
    let (client_end, server_end) = tokio::io::duplex(1024);
    drop(server_end);
    let (reader, writer) = tokio::io::split(client_end);
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(
        loopal_ipc::StdioTransport::new(Box::new(BufReader::new(reader)), Box::new(writer)),
    );
    let (connection, _incoming) =
        loopal_ipc::connection::Connection::new(transport).into_listening();
    SessionController::with_hub(Arc::new(HubClient::new(connection)))
}
