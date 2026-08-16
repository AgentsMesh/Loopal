use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::{McpServerConfig, McpSharing};
use rmcp::model::{ClientJsonRpcMessage, ClientRequest, PingRequest, RequestId};
use rmcp::transport::streamable_http_client::{StreamableHttpClient, StreamableHttpError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::ScopedHttpClient;
use super::tests::{RotatingClient, provenance};

fn config(url: String, value: &str) -> McpServerConfig {
    McpServerConfig::StreamableHttp {
        url,
        headers: HashMap::from([("X-API-Key".into(), value.into())]),
        enabled: true,
        timeout_ms: 1_000,
        sharing: McpSharing::HubSingleton,
    }
}

fn ping() -> ClientJsonRpcMessage {
    ClientJsonRpcMessage::request(
        ClientRequest::PingRequest(PingRequest::default()),
        RequestId::Number(1),
    )
}

#[tokio::test]
async fn same_origin_307_preserves_config_owned_api_key() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let next = format!("{url}/next");
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let initial = read_request(&mut first).await;
        write_response(&mut first, &redirect(&next)).await;
        let (mut second, _) = listener.accept().await.unwrap();
        let redirected = read_request(&mut second).await;
        write_response(&mut second, &accepted()).await;
        (initial, redirected)
    });
    let client = ScopedHttpClient::new(config(url.clone(), "ordinary-key"), None, provenance());

    client
        .post_message(url.into(), ping(), None, None, HashMap::new())
        .await
        .unwrap();

    let (initial, redirected) = server.await.unwrap();
    assert!(has_header(&initial, "x-api-key", "ordinary-key"));
    assert!(has_header(&redirected, "x-api-key", "ordinary-key"));
}

#[tokio::test]
async fn cross_origin_307_denies_redirect_before_secret_api_key_can_leak() {
    let source = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", source.local_addr().unwrap());
    let target_url = format!("http://{}/capture", target.local_addr().unwrap());
    let source_server = tokio::spawn(async move {
        let (mut socket, _) = source.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        write_response(&mut socket, &redirect(&target_url)).await;
        request
    });
    let secret_client = Arc::new(RotatingClient {
        calls: std::sync::atomic::AtomicUsize::new(0),
        rotate: false,
    });
    let client = ScopedHttpClient::new(
        config(url.clone(), "{{secret:token}}"),
        Some(secret_client),
        provenance(),
    );

    let error = client
        .post_message(url.into(), ping(), None, None, HashMap::new())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("redirect"));
    assert!(has_header(
        &source_server.await.unwrap(),
        "x-api-key",
        "secret-1"
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), target.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn redirects_with_a_different_scheme_or_host_are_denied() {
    for target in [RedirectTarget::Https, RedirectTarget::Localhost] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}/mcp");
        let target_url = match target {
            RedirectTarget::Https => format!("https://{address}/capture"),
            RedirectTarget::Localhost => format!("http://localhost:{}/capture", address.port()),
        };
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let _ = read_request(&mut socket).await;
            write_response(&mut socket, &redirect(&target_url)).await;
        });
        let client = ScopedHttpClient::new(config(url.clone(), "ordinary-key"), None, provenance());

        let error = client
            .post_message(url.into(), ping(), None, None, HashMap::new())
            .await
            .unwrap_err();

        assert!(error.to_string().contains("redirect"));
        server.await.unwrap();
    }
}

#[derive(Clone, Copy)]
enum RedirectTarget {
    Https,
    Localhost,
}

#[tokio::test]
async fn same_origin_redirect_chain_is_bounded() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let next = url.clone();
    let server = tokio::spawn(async move {
        for _ in 0..=10 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let _ = read_request(&mut socket).await;
            write_response(&mut socket, &redirect(&next)).await;
        }
    });
    let client = ScopedHttpClient::new(config(url.clone(), "ordinary-key"), None, provenance());

    let error = client
        .post_message(url.into(), ping(), None, None, HashMap::new())
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        StreamableHttpError::Client(ref error) if error.is_redirect()
    ));
    server.await.unwrap();
}

fn redirect(location: &str) -> String {
    format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

fn accepted() -> String {
    "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()
}

fn has_header(request: &str, name: &str, value: &str) -> bool {
    request.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(actual, rest)| actual.eq_ignore_ascii_case(name) && rest.trim() == value)
    })
}

async fn write_response(socket: &mut TcpStream, response: &str) {
    socket.write_all(response.as_bytes()).await.unwrap();
    socket.shutdown().await.unwrap();
}

async fn read_request(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = socket.read(&mut buffer).await.unwrap();
        bytes.extend_from_slice(&buffer[..read]);
        if read == 0 || bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
