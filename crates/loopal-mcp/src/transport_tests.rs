use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use loopal_config::{McpServerConfig, McpSharing};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::Mutex;

use super::{connect_http, connect_stdio, drain_stderr, is_auth_error, spawn_stderr_drain};
use crate::handshake_transport::HandshakePolicy;
use crate::safe_diagnostics::REDACTED_STDERR;
use crate::scoped_http_client::ScopedHttpClient;
use crate::secret_provenance::SecretProvenance;

#[test]
fn classifies_only_authentication_errors() {
    for message in ["authentication required", "HTTP 401", "UNAUTHORIZED"] {
        assert!(is_auth_error(&loopal_error::McpError::ConnectionFailed(
            message.into()
        )));
    }
    assert!(!is_auth_error(&loopal_error::McpError::ConnectionFailed(
        "ordinary failure".into()
    )));
}

#[tokio::test]
async fn public_stdio_and_http_failures_are_fixed_and_safe() {
    let marker = "exact-plaintext";
    let stdio = match connect_stdio(
        "__missing_mcp_binary__",
        &[],
        &HashMap::new(),
        Duration::from_millis(50),
        None,
        None,
    )
    .await
    {
        Ok(_) => panic!("missing stdio server connected"),
        Err(error) => error,
    };
    assert_eq!(
        stdio.to_string(),
        "Connection failed: MCP stdio spawn failed"
    );

    let (url, _) = crate::http_test_support::server(vec![crate::http_test_support::response(
        "500 Internal Server Error",
        "text/plain",
        marker,
    )])
    .await;
    let config = McpServerConfig::StreamableHttp {
        url: url.clone(),
        headers: HashMap::new(),
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
    };
    let client = ScopedHttpClient::new(config, None, Arc::new(SecretProvenance::default()));
    let error = match connect_http(
        &url,
        client,
        Duration::from_secs(1),
        None,
        HandshakePolicy::Strip,
    )
    .await
    {
        Ok(_) => panic!("invalid HTTP server connected"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "Connection failed: MCP HTTP connection failed"
    );
    assert!(!format!("{error:?}").contains(marker));
}

#[tokio::test]
async fn stderr_records_only_one_bounded_marker() {
    let mut initial = VecDeque::with_capacity(16);
    for index in 0..16 {
        initial.push_back(format!("old-{index}"));
    }
    let tail = Arc::new(Mutex::new(initial));

    drain_stderr(&b"exact-plaintext\nsecond line"[..], Some(tail.clone())).await;

    let tail = tail.lock().await;
    assert_eq!(tail.len(), 16);
    assert_eq!(tail.front().map(String::as_str), Some("old-1"));
    assert_eq!(tail.back().map(String::as_str), Some(REDACTED_STDERR));
    assert!(!format!("{tail:?}").contains("exact-plaintext"));
}

#[tokio::test]
async fn spawned_drain_records_marker_without_retaining_plaintext() {
    use tokio::io::AsyncWriteExt;

    let (reader, mut writer) = tokio::io::duplex(64);
    let tail = Arc::new(Mutex::new(VecDeque::with_capacity(16)));
    spawn_stderr_drain(reader, Some(tail.clone()));
    writer.write_all(b"exact-plaintext").await.unwrap();
    writer.shutdown().await.unwrap();

    for _ in 0..20 {
        if !tail.lock().await.is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(tail.lock().await.back().unwrap(), REDACTED_STDERR);
}

#[tokio::test]
async fn whitespace_errors_and_absent_tail_add_no_diagnostics() {
    let tail = Arc::new(Mutex::new(VecDeque::with_capacity(16)));
    drain_stderr(&b" \n\t"[..], Some(tail.clone())).await;
    drain_stderr(ErrorReader, Some(tail.clone())).await;
    drain_stderr(&b"plaintext"[..], None).await;
    assert!(tail.lock().await.is_empty());
}

#[tokio::test]
async fn stderr_detects_non_whitespace_after_an_initial_whitespace_chunk() {
    let tail = Arc::new(Mutex::new(VecDeque::with_capacity(16)));
    let reader = ChunkReader {
        chunks: VecDeque::from([
            b" \n\t".as_slice(),
            b"exact-plaintext".as_slice(),
            b"additional plaintext".as_slice(),
        ]),
    };

    drain_stderr(reader, Some(tail.clone())).await;

    let tail = tail.lock().await;
    assert_eq!(tail.len(), 1);
    assert_eq!(tail.back().map(String::as_str), Some(REDACTED_STDERR));
    assert!(!format!("{tail:?}").contains("exact-plaintext"));
}

struct ChunkReader {
    chunks: VecDeque<&'static [u8]>,
}

impl AsyncRead for ChunkReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if let Some(chunk) = self.chunks.pop_front() {
            buffer.put_slice(chunk);
        }
        Poll::Ready(Ok(()))
    }
}

struct ErrorReader;

impl AsyncRead for ErrorReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Err(std::io::Error::other("read failed")))
    }
}
