use super::stream_helpers::{collect_chunks, test_chat_params};
use loopal_error::{LoopalError, ProviderError};
use loopal_provider::{AnthropicProvider, GoogleProvider, OpenAiCompatProvider, OpenAiProvider};
use loopal_provider_api::{Provider, StreamChunk};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn openai_failed(error: &str) -> LoopalError {
    let server = MockServer::start().await;
    let sse =
        format!("data: {{\"type\":\"response.failed\",\"response\":{{\"error\":{error}}}}}\n\n");
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;
    let provider = OpenAiProvider::new("key".into()).with_base_url(server.uri());
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;
    chunks.into_iter().next().expect("error chunk").unwrap_err()
}

#[tokio::test]
async fn responses_failed_preserves_context_overflow() {
    let error =
        openai_failed(r#"{"code":"context_length_exceeded","message":"maximum context length"}"#)
            .await;
    assert!(matches!(
        error,
        LoopalError::Provider(ProviderError::ContextOverflow { .. })
    ));
}

#[tokio::test]
async fn responses_failed_preserves_retryable_server_error() {
    let error = openai_failed(r#"{"code":"server_error","message":"internal"}"#).await;
    assert!(
        error.is_retryable(),
        "server_error must retain retry semantics"
    );
    assert!(matches!(
        error,
        LoopalError::Provider(ProviderError::Api { status: 500, .. })
    ));
}

#[tokio::test]
async fn responses_failed_preserves_rate_limit() {
    let error = openai_failed(r#"{"code":"rate_limit_exceeded","message":"slow down"}"#).await;
    assert!(matches!(
        error,
        LoopalError::Provider(ProviderError::RateLimited {
            retry_after_ms: 30_000
        })
    ));
}

#[tokio::test]
async fn google_explicit_block_reasons_fail_closed_without_done() {
    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"blocked\"}]},",
        "\"finishReason\":\"SAFETY\"}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"quoted\"}]},",
        "\"finishReason\":\"RECITATION\"}]}\n\n",
        "data: {\"promptFeedback\":{\"blockReason\":\"SAFETY\"}}\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/models/test-model:streamGenerateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;
    let provider = GoogleProvider::new("key".into()).with_base_url(server.uri());
    let chunks = collect_chunks(provider.stream_chat(&test_chat_params()).await.unwrap()).await;
    assert!(
        chunks
            .iter()
            .all(|chunk| !matches!(chunk, Ok(StreamChunk::Done { .. }))),
        "blocked responses cannot carry a successful terminal marker: {chunks:?}"
    );
    let errors = chunks
        .iter()
        .filter_map(|chunk| chunk.as_ref().err())
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 3, "chunks: {chunks:?}");
    assert!(errors.iter().all(|error| {
        !error.is_retryable()
            && matches!(
                error,
                LoopalError::Provider(ProviderError::Api { status: 400, .. })
            )
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("RECITATION"))
    );
}

#[tokio::test]
async fn compat_reasoning_content_requires_catalog_capability() {
    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"plan\",",
        "\"content\":\"answer\"},\"finish_reason\":\"stop\"}]}\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;
    let provider = OpenAiCompatProvider::new("key".into(), server.uri(), "compat".into());

    let mut reasoning = test_chat_params();
    reasoning.model = "deepseek-reasoner".into();
    let enabled = collect_chunks(provider.stream_chat(&reasoning).await.unwrap()).await;
    assert!(
        enabled
            .iter()
            .any(|chunk| matches!(chunk, Ok(StreamChunk::Thinking { text }) if text == "plan"))
    );

    let mut plain = test_chat_params();
    plain.model = "deepseek-chat".into();
    let disabled = collect_chunks(provider.stream_chat(&plain).await.unwrap()).await;
    assert!(
        !disabled
            .iter()
            .any(|chunk| matches!(chunk, Ok(StreamChunk::Thinking { .. })))
    );
}

#[tokio::test]
async fn every_http_adapter_preserves_retry_after_on_504() {
    let server = MockServer::start().await;
    for endpoint in [
        "/v1/messages",
        "/v1/responses",
        "/v1/chat/completions",
        "/models/test-model:streamGenerateContent",
    ] {
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(504)
                    .insert_header("retry-after", "1.25")
                    .set_body_string("gateway timeout"),
            )
            .mount(&server)
            .await;
    }

    let providers: Vec<Box<dyn Provider>> = vec![
        Box::new(AnthropicProvider::new("key".into()).with_base_url(server.uri())),
        Box::new(OpenAiProvider::new("key".into()).with_base_url(server.uri())),
        Box::new(OpenAiCompatProvider::new(
            "key".into(),
            server.uri(),
            "compat".into(),
        )),
        Box::new(GoogleProvider::new("key".into()).with_base_url(server.uri())),
    ];

    for provider in providers {
        let error = match provider.stream_chat(&test_chat_params()).await {
            Ok(_) => panic!("{} unexpectedly accepted a 504", provider.name()),
            Err(error) => error,
        };
        assert!(error.is_retryable(), "provider={}", provider.name());
        assert_eq!(
            error.retry_after_ms(),
            Some(1_250),
            "provider={}",
            provider.name()
        );
        assert!(matches!(
            error,
            LoopalError::Provider(ProviderError::Api { status: 504, .. })
        ));
    }
}
