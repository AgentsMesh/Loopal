use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::Connection;
use loopal_ipc::rpc_error::RpcError;
use loopal_ipc::transport::Transport;
use serde_json::json;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn rpc_error_preserves_data_field() {
    let (server_to_client_tx, server_to_client_rx) = tokio::io::duplex(4096);
    let (client_to_server_tx, _client_to_server_rx) = tokio::io::duplex(4096);

    let client_transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(server_to_client_rx)),
        Box::new(client_to_server_tx),
    ));
    let (client, _rx) = Connection::new(client_transport).into_listening();

    let mut server_writer = server_to_client_tx;
    let response_with_data = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -32603,
            "message": "session start failed",
            "data": {
                "missing_field": "session_id",
                "available_sessions": ["abc", "def"]
            }
        }
    });
    let mut line = serde_json::to_vec(&response_with_data).unwrap();
    line.push(b'\n');
    server_writer.write_all(&line).await.unwrap();
    server_writer.flush().await.unwrap();

    let result = client.send_request("any_method", json!({})).await;
    let err = result.expect_err("error response should be Err");

    match err {
        RpcError::Remote {
            code,
            message,
            data,
        } => {
            assert_eq!(code, -32603);
            assert_eq!(message, "session start failed");
            let data = data.expect("data field must be preserved end-to-end");
            assert_eq!(data["missing_field"], "session_id");
            assert_eq!(data["available_sessions"][0], "abc");
        }
        other => panic!("expected Remote, got: {other:?}"),
    }
}

#[tokio::test]
async fn rpc_error_display_includes_code_and_message() {
    let err = RpcError::Remote {
        code: -32601,
        message: "method not found".into(),
        data: None,
    };
    let displayed = err.to_string();
    assert!(displayed.contains("-32601"), "got: {displayed}");
    assert!(displayed.contains("method not found"), "got: {displayed}");
}

#[tokio::test]
async fn rpc_error_remote_code_accessor() {
    let remote = RpcError::Remote {
        code: -42,
        message: "x".into(),
        data: None,
    };
    assert_eq!(remote.remote_code(), Some(-42));

    let transport = RpcError::Transport("boom".into());
    assert_eq!(transport.remote_code(), None);

    let dropped = RpcError::ChannelDropped;
    assert_eq!(dropped.remote_code(), None);
}
