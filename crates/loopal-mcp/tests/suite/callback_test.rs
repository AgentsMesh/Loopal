//! Tests for OAuth callback URL parsing and percent-decoding.

use loopal_mcp::oauth::callback::{self};

// parse_callback_params is private, so we test through start_callback_server
// indirectly. But the percent_decode logic can be tested via the public API
// by actually running the callback server.

#[tokio::test]
async fn test_callback_server_starts_and_returns_port() {
    let (port, _rx) = callback::start_callback_server()
        .await
        .expect("server start failed");
    assert!(port > 0);
    // The rx is a oneshot receiver; dropping it is fine.
}

#[tokio::test]
async fn test_callback_server_receives_params() {
    let (port, rx) = callback::start_callback_server()
        .await
        .expect("server start failed");

    // Simulate browser redirect by connecting as a TCP client.
    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect failed");

    let request = format!(
        "GET /oauth_callback?code=abc123&state=xyz789 HTTP/1.1\r\n\
         Host: localhost:{port}\r\n\r\n"
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, request.as_bytes())
        .await
        .expect("write failed");

    let params = tokio::time::timeout(std::time::Duration::from_secs(3), rx)
        .await
        .expect("timeout waiting for params")
        .expect("channel closed");

    assert_eq!(params.code, "abc123");
    assert_eq!(params.state, "xyz789");
}

#[tokio::test]
async fn test_callback_server_percent_decoding() {
    let (port, rx) = callback::start_callback_server()
        .await
        .expect("server start failed");

    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect failed");

    // code contains URL-encoded characters
    let request = format!(
        "GET /oauth_callback?code=abc%2F123%3D&state=x%20y HTTP/1.1\r\n\
         Host: localhost:{port}\r\n\r\n"
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, request.as_bytes())
        .await
        .expect("write failed");

    let params = tokio::time::timeout(std::time::Duration::from_secs(3), rx)
        .await
        .expect("timeout")
        .expect("channel closed");

    assert_eq!(params.code, "abc/123=");
    assert_eq!(params.state, "x y");
}

#[tokio::test]
async fn test_callback_server_missing_params_returns_400() {
    let (port, rx) = callback::start_callback_server()
        .await
        .expect("server start failed");

    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect failed");

    // Missing state parameter
    let request = format!(
        "GET /oauth_callback?code=abc HTTP/1.1\r\n\
         Host: localhost:{port}\r\n\r\n"
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, request.as_bytes())
        .await
        .expect("write failed");

    // Read response — should be 400
    let mut buf = vec![0u8; 1024];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .unwrap_or(0);
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("400 Bad Request"));

    // rx should NOT have received anything
    drop(rx);
}
