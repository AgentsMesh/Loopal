use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use rmcp::transport::streamable_http_client::{StreamableHttpClient, StreamableHttpPostResponse};

use super::OAuthObservingHttpClient;
use super::test_support::{TestClient, error_message, ping};
use crate::oauth_credential_seed::{OAUTH_RESPONSE_DENIED, OAuthCredentialSeed};

#[tokio::test]
async fn observes_rotations_across_all_authenticated_methods() {
    let post_token = "post-access-token";
    let stream_token = "stream-access-token";
    let delete_token = "delete-access-token";
    let inner = TestClient::new(vec![Ok(StreamableHttpPostResponse::Json(
        error_message(post_token),
        None,
    ))]);
    let observed = inner.observed.clone();
    let credentials = Arc::new(OAuthCredentialSeed::default());
    let client = OAuthObservingHttpClient::new(inner, credentials.clone());

    let response = client
        .post_message(
            "http://test".into(),
            ping(),
            None,
            Some(post_token.into()),
            HashMap::new(),
        )
        .await
        .unwrap();
    assert!(!format!("{response:?}").contains(post_token));

    let mut stream = client
        .get_stream(
            "http://test".into(),
            "session".into(),
            None,
            Some(stream_token.into()),
            HashMap::new(),
        )
        .await
        .unwrap();
    assert!(stream.next().await.is_none());
    client
        .delete_session(
            "http://test".into(),
            "session".into(),
            Some(delete_token.into()),
            HashMap::new(),
        )
        .await
        .unwrap();

    assert_eq!(
        *observed.lock().unwrap(),
        [
            ("post", Some(post_token.into())),
            ("get", Some(stream_token.into())),
            ("delete", Some(delete_token.into())),
        ]
    );
    let redactor = credentials.redactor().unwrap();
    let (redacted, _) = redactor.scan_and_redact(&format!(
        "{post_token} {stream_token} Bearer {delete_token}"
    ));
    assert!(!redacted.contains("access-token"));
}

#[tokio::test]
async fn strips_server_error_text_and_rejects_secret_session_ids() {
    let token = "oauth-access-token";
    let inner = TestClient::new(vec![
        Err(format!("server error {token}")),
        Ok(StreamableHttpPostResponse::Json(
            error_message("ordinary"),
            Some(token.into()),
        )),
        Ok(StreamableHttpPostResponse::Json(
            error_message("ordinary"),
            Some("ordinary-session".into()),
        )),
    ]);
    let credentials = Arc::new(OAuthCredentialSeed::default());
    let client = OAuthObservingHttpClient::new(inner, credentials);

    let error = client
        .post_message(
            "http://test".into(),
            ping(),
            None,
            Some(token.into()),
            HashMap::new(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains(OAUTH_RESPONSE_DENIED));
    assert!(!error.to_string().contains(token));

    let error = client
        .post_message(
            "http://test".into(),
            ping(),
            None,
            Some(token.into()),
            HashMap::new(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains(OAUTH_RESPONSE_DENIED));

    let response = client
        .post_message(
            "http://test".into(),
            ping(),
            None,
            Some(token.into()),
            HashMap::new(),
        )
        .await
        .unwrap();
    assert!(!format!("{response:?}").contains(token));
}

#[tokio::test]
async fn sanitizes_post_sse_fields_plain_data_and_json_arrays() {
    let token = "sse-access-token";
    let body = format!(
        "event: {token}\nid: Bearer {token}\ndata: {}\n\ndata: plain {token}\n\ndata: [\"{token}\",{{\"{token}\":\"{token}\"}}]\n\n",
        serde_json::to_string(&error_message(token)).unwrap()
    );
    let (url, _) = crate::http_test_support::server(vec![crate::http_test_support::response(
        "200 OK",
        "text/event-stream",
        &body,
    )])
    .await;
    let credentials = Arc::new(OAuthCredentialSeed::default());
    let client = OAuthObservingHttpClient::new(reqwest::Client::new(), credentials);

    let response = client
        .post_message(url.into(), ping(), None, Some(token.into()), HashMap::new())
        .await
        .unwrap();
    let StreamableHttpPostResponse::Sse(mut stream, _) = response else {
        panic!("expected SSE")
    };
    let mut encoded = String::new();
    while let Some(event) = stream.next().await {
        encoded.push_str(&format!("{:?}", event.unwrap()));
    }
    assert!(!encoded.contains(token));
    assert!(encoded.contains("<secret_ref:mcp_oauth"));
    assert!(encoded.contains("plain <secret_ref:mcp_oauth_access_token>"));
}
