use loopal_backend::ResourceLimits;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

async fn spawn_capture_server() -> (String, Arc<Mutex<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_writer = captured.clone();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let mut total = Vec::new();
        loop {
            let n = socket.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            total.extend_from_slice(&buf[..n]);
            if total.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        *captured_writer.lock().await = total;
        let _ = socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
            )
            .await;
        let _ = socket.shutdown().await;
    });
    (format!("http://{addr}/"), captured)
}

async fn spawn_static_server(response: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let mut total = Vec::new();
        loop {
            let n = socket.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            total.extend_from_slice(&buf[..n]);
            if total.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let _ = socket.write_all(&response).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}/")
}

#[tokio::test]
async fn fetch_sends_browser_user_agent_and_accept_language() {
    let (url, captured) = spawn_capture_server().await;
    let limits = ResourceLimits::default();
    let result = loopal_backend::net::fetch_url(&url, None, &limits).await;
    assert!(result.is_ok(), "fetch should succeed: {:?}", result.err());
    let raw = captured.lock().await.clone();
    let req = String::from_utf8_lossy(&raw).to_ascii_lowercase();
    assert!(
        req.contains("user-agent: mozilla/"),
        "outgoing request must carry a browser User-Agent; got: {req}"
    );
    assert!(
        req.contains("accept-language: zh-cn"),
        "outgoing request must carry Accept-Language; got: {req}"
    );
    assert!(
        req.contains("accept: text/html"),
        "outgoing request must carry Accept declaring HTML; got: {req}"
    );
}

#[tokio::test]
async fn fetch_returns_error_on_anti_bot_challenge_page() {
    // Baidu-style anti-bot stub: HTTP 200 but body is the enablejs trampoline.
    let body = "<html><meta http-equiv=\"refresh\" \
                content=\"0;url=/httpservice/retry/enablejs?sei=abc\"></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes();
    let url = spawn_static_server(response).await;
    let limits = ResourceLimits::default();
    let result = loopal_backend::net::fetch_url(&url, None, &limits).await;
    let err = result.expect_err("anti-bot stub must surface as error, not success with noise");
    let msg = err.to_string().to_ascii_lowercase();
    assert!(
        msg.contains("anti-bot") && msg.contains("enablejs"),
        "error should name anti-bot and the matched marker; got: {msg}"
    );
}

#[tokio::test]
async fn fetch_reports_final_url_when_redirect_crosses_host() {
    // Stage 2: real target the redirect points at.
    let stage2_response =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok".to_vec();
    let stage2_url = spawn_static_server(stage2_response).await;
    // Swap "127.0.0.1" for "localhost" so reqwest sees a different host_str.
    let stage2_host_alias = stage2_url.replace("127.0.0.1", "localhost");
    let stage1_response =
        format!("HTTP/1.1 302 Found\r\nLocation: {stage2_host_alias}\r\nContent-Length: 0\r\n\r\n")
            .into_bytes();
    let stage1_url = spawn_static_server(stage1_response).await;
    let limits = ResourceLimits::default();
    let result = loopal_backend::net::fetch_url(&stage1_url, None, &limits)
        .await
        .expect("redirect chain should succeed");
    let final_url = result.final_url.expect(
        "cross-host redirect must populate final_url so callers can decide whether to follow",
    );
    assert!(
        final_url.contains("localhost"),
        "final_url should reflect the redirected host; got: {final_url}"
    );
}

#[tokio::test]
async fn fetch_rejects_non_http_schemes_before_connecting() {
    let error =
        loopal_backend::net::fetch_url("file:///etc/passwd", None, &ResourceLimits::default())
            .await
            .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("must start with http:// or https://")
    );
}

#[tokio::test]
async fn fetch_rejects_non_success_statuses() {
    let url = spawn_static_server(
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n".to_vec(),
    )
    .await;

    let error = loopal_backend::net::fetch_url(&url, None, &ResourceLimits::default())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("HTTP 503"));
}

#[tokio::test]
async fn fetch_truncates_at_the_byte_cap_and_allows_missing_content_type() {
    let url =
        spawn_static_server(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabcdefghij".to_vec())
            .await;
    let limits = ResourceLimits {
        max_fetch_bytes: 4,
        ..ResourceLimits::default()
    };

    let result = loopal_backend::net::fetch_url(&url, None, &limits)
        .await
        .unwrap();

    assert_eq!(result.body, "abcd");
    assert_eq!(result.content_type, None);
    assert_eq!(result.final_url, None);
}

#[tokio::test]
async fn fetch_omits_final_url_for_redirects_that_keep_the_same_host() {
    let target =
        spawn_static_server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec()).await;
    let redirect = spawn_static_server(
        format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n")
            .into_bytes(),
    )
    .await;

    let result = loopal_backend::net::fetch_url(&redirect, None, &ResourceLimits::default())
        .await
        .unwrap();

    assert_eq!(result.body, "ok");
    assert_eq!(result.final_url, None);
}

#[tokio::test]
async fn fetch_surfaces_incomplete_response_bodies_as_read_errors() {
    let url =
        spawn_static_server(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec()).await;

    let error = loopal_backend::net::fetch_url(&url, None, &ResourceLimits::default())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("read error"));
}
