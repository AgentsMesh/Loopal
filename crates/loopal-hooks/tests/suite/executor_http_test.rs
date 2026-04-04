//! Tests for HttpExecutor using a local TCP server.

use std::time::Duration;

use loopal_hooks::executor::HookExecutor;
use loopal_hooks::executor_http::HttpExecutor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn a minimal HTTP server that returns the given status and body.
async fn spawn_http_server(status: u16, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let body = body.to_string();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let _ = stream.read(&mut buf).await; // consume request
        let response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn test_http_executor_success_200() {
    let url = spawn_http_server(200, r#"{"additional_context":"ok"}"#).await;
    let exec = HttpExecutor {
        url,
        headers: Default::default(),
        timeout: Duration::from_secs(5),
    };
    let result = exec.execute(serde_json::json!({"tool": "Write"})).await;
    let output = result.unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("additional_context"));
}

#[tokio::test]
async fn test_http_executor_error_500() {
    let url = spawn_http_server(500, "internal error").await;
    let exec = HttpExecutor {
        url,
        headers: Default::default(),
        timeout: Duration::from_secs(5),
    };
    let result = exec.execute(serde_json::json!({})).await;
    let output = result.unwrap();
    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("internal error"));
}

#[tokio::test]
async fn test_http_executor_timeout() {
    // Server never responds
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut _stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(60)).await; // hang
    });

    let exec = HttpExecutor {
        url: format!("http://{addr}"),
        headers: Default::default(),
        timeout: Duration::from_millis(100), // very short
    };
    let result = exec.execute(serde_json::json!({})).await;
    assert!(result.is_err());
}
