use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_config::{McpServerConfig, McpSharing};
use loopal_secret_client::{IpcBudget, SecretClient, SecretResult, SecretString};
use rmcp::model::{ClientJsonRpcMessage, ClientRequest, PingRequest, RequestId};
use rmcp::transport::streamable_http_client::StreamableHttpClient;

use super::ScopedHttpClient;
use crate::http_test_support::{response, server};
use crate::secret_expand::CONFIG_SECRET_ERROR;
use crate::secret_provenance::SecretProvenance;

pub(super) fn provenance() -> Arc<SecretProvenance> {
    Arc::new(SecretProvenance::default())
}

pub(super) struct RotatingClient {
    pub(super) calls: AtomicUsize,
    pub(super) rotate: bool,
}

#[async_trait]
impl SecretClient for RotatingClient {
    async fn get(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let version = if self.rotate { call } else { 1 };
        Ok(SecretString::from(format!("secret-{version}")))
    }

    async fn list_names(&self, _: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn expand_author(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }

    async fn expand_wire(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }
}

pub(super) fn config(header: &str, value: &str) -> McpServerConfig {
    McpServerConfig::StreamableHttp {
        url: "https://example.test/mcp".into(),
        headers: HashMap::from([(header.into(), value.into())]),
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
    }
}

#[tokio::test]
async fn resolves_config_headers_for_each_request_without_caching_plaintext() {
    let secrets = Arc::new(RotatingClient {
        calls: AtomicUsize::new(0),
        rotate: false,
    });
    let client = ScopedHttpClient::new(
        config("x-loopal-token", "Bearer {{secret:token}}"),
        Some(secrets.clone()),
        provenance(),
    );

    let first = client.request_headers().await.unwrap();
    let second = client.request_headers().await.unwrap();

    let header = reqwest::header::HeaderName::from_static("x-loopal-token");
    assert_eq!(first[&header].to_str().unwrap(), "Bearer secret-1");
    assert_eq!(second[&header].to_str().unwrap(), "Bearer secret-1");
    assert_eq!(secrets.calls.load(Ordering::SeqCst), 2);
    let McpServerConfig::StreamableHttp { headers, .. } = &client.config else {
        unreachable!()
    };
    assert_eq!(headers["x-loopal-token"], "Bearer {{secret:token}}");
}

#[tokio::test]
async fn secret_rotation_fails_closed_before_building_a_request() {
    let secrets = Arc::new(RotatingClient {
        calls: AtomicUsize::new(0),
        rotate: true,
    });
    let client = ScopedHttpClient::new(
        config("x-loopal-token", "{{secret:token}}"),
        Some(secrets),
        provenance(),
    );
    client.request_headers().await.unwrap();
    let error = client.request_headers().await.unwrap_err().to_string();
    assert!(error.contains(CONFIG_SECRET_ERROR));
    assert!(!error.contains("secret-2"));
}

#[tokio::test]
async fn every_http_method_resolves_a_fresh_secret_for_the_actual_request() {
    let (url, requests) = server(vec![
        response("202 Accepted", "application/json", ""),
        response("204 No Content", "application/json", ""),
        response("200 OK", "text/event-stream", "event: ping\n\n"),
    ])
    .await;
    let secrets = Arc::new(RotatingClient {
        calls: AtomicUsize::new(0),
        rotate: false,
    });
    let client = ScopedHttpClient::new(
        config("x-loopal-token", "{{secret:token}}"),
        Some(secrets.clone()),
        provenance(),
    );
    let message = ClientJsonRpcMessage::request(
        ClientRequest::PingRequest(PingRequest::default()),
        RequestId::Number(1),
    );
    let uri: Arc<str> = url.into();

    client
        .post_message(uri.clone(), message, None, None, HashMap::new())
        .await
        .unwrap();
    client
        .delete_session(uri.clone(), Arc::from("session"), None, HashMap::new())
        .await
        .unwrap();
    let _stream = client
        .get_stream(uri, Arc::from("session"), None, None, HashMap::new())
        .await
        .unwrap();

    let requests = requests.lock().await;
    assert_eq!(
        requests
            .iter()
            .map(|request| (request.method.as_str(), request.token.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("POST", Some("secret-1")),
            ("DELETE", Some("secret-1")),
            ("GET", Some("secret-1")),
        ]
    );
    assert_eq!(secrets.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn missing_secret_client_and_invalid_headers_fail_with_safe_error() {
    for client in [
        ScopedHttpClient::new(
            config("authorization", "{{secret:token}}"),
            None,
            provenance(),
        ),
        ScopedHttpClient::new(config("invalid header", "ordinary"), None, provenance()),
        ScopedHttpClient::new(config("x-valid", "invalid\nvalue"), None, provenance()),
        ScopedHttpClient::new(
            McpServerConfig::Stdio {
                command: "fixture".into(),
                args: Vec::new(),
                env: HashMap::new(),
                enabled: true,
                timeout_ms: 100,
                sharing: McpSharing::HubSingleton,
                cwd_isolation: None,
            },
            None,
            provenance(),
        ),
    ] {
        let error = match client.request_headers().await {
            Ok(_) => panic!("invalid HTTP secret config accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(CONFIG_SECRET_ERROR));
        assert!(!error.contains("token"));
    }
}
