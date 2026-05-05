use std::sync::Arc;

use async_trait::async_trait;
use loopal_tool_api::{FetchRefinerPolicy, OneShotChatError, OneShotChatService, ToolContext};
use loopal_tool_fetch::__try_refine_internal as try_refine;

#[derive(Clone, Default)]
struct MockResolver {
    model: Option<String>,
    response: Option<Result<String, OneShotChatError>>,
    record: Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl OneShotChatService for MockResolver {
    async fn one_shot_chat(
        &self,
        model: &str,
        _system: &str,
        user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        self.record
            .lock()
            .unwrap()
            .push(format!("model={model}|user={}", user_prompt.len()));
        self.response
            .clone()
            .unwrap_or(Err(OneShotChatError::EmptyResponse))
    }
}

impl FetchRefinerPolicy for MockResolver {
    fn refiner_model(&self, _body_size: usize) -> Option<String> {
        self.model.clone()
    }
}

fn make_ctx_with_resolver(resolver: Option<Arc<MockResolver>>) -> (ToolContext, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let backend = loopal_backend::LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    let ctx = ToolContext::new(backend, "t");
    let ctx = match resolver {
        Some(m) => ctx
            .with_one_shot_chat(m.clone())
            .with_fetch_refiner_policy(m),
        None => ctx,
    };
    (ctx, tmp)
}

fn big_body(byte_count: usize) -> String {
    "x".repeat(byte_count)
}

#[tokio::test]
async fn refiner_triggers_above_threshold() {
    let resolver = Arc::new(MockResolver {
        model: Some("haiku-test".into()),
        response: Some(Ok("## Direct Answer\nFound the auth header.".into())),
        ..Default::default()
    });
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(10_000);
    let r = try_refine(&ctx, "find auth header", "https://example.com/api", &body)
        .await
        .expect("refine should fire");
    assert!(!r.is_error);
    assert!(r.content.contains("[Refined for: find auth header]"));
    assert!(r.content.contains("source: https://example.com/api"));
    assert!(r.content.contains("--- summary ---"));
    assert!(r.content.contains("Found the auth header."));
}

#[tokio::test]
async fn refiner_skips_when_resolver_returns_no_model() {
    let resolver = Arc::new(MockResolver {
        model: None,
        response: Some(Ok("ignored".into())),
        ..Default::default()
    });
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(10_000);
    let r = try_refine(&ctx, "intent", "https://example.com/", &body).await;
    assert!(r.is_none(), "no model → fetch must not refine");
}

#[tokio::test]
async fn refiner_skips_when_no_resolver() {
    let (ctx, _tmp) = make_ctx_with_resolver(None);
    let body = big_body(10_000);
    let r = try_refine(&ctx, "intent", "https://example.com/", &body).await;
    assert!(r.is_none(), "no resolver → fetch must not refine");
}

#[tokio::test]
async fn refiner_falls_back_when_chat_errors() {
    let resolver = Arc::new(MockResolver {
        model: Some("haiku-test".into()),
        response: Some(Err(OneShotChatError::Timeout)),
        ..Default::default()
    });
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(10_000);
    let r = try_refine(&ctx, "intent", "https://example.com/", &body).await;
    assert!(r.is_none(), "Timeout → caller falls back");
}

#[tokio::test]
async fn refiner_falls_back_when_provider_unresolvable() {
    let resolver = Arc::new(MockResolver {
        model: Some("nonexistent".into()),
        response: Some(Err(OneShotChatError::ProviderUnresolvable)),
        ..Default::default()
    });
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(10_000);
    let r = try_refine(&ctx, "intent", "https://example.com/", &body).await;
    assert!(r.is_none(), "ProviderUnresolvable → caller falls back");
}

#[tokio::test]
async fn refiner_falls_back_when_chat_returns_empty() {
    let resolver = Arc::new(MockResolver {
        model: Some("haiku-test".into()),
        response: Some(Err(OneShotChatError::EmptyResponse)),
        ..Default::default()
    });
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(10_000);
    let r = try_refine(&ctx, "intent", "https://example.com/", &body).await;
    assert!(r.is_none(), "EmptyResponse → caller falls back");
}

#[tokio::test]
async fn refiner_records_user_intent_in_request() {
    let resolver = Arc::new(MockResolver {
        model: Some("haiku-test".into()),
        response: Some(Ok("ok".into())),
        ..Default::default()
    });
    let record = resolver.record.clone();
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(10_000);
    let _ = try_refine(&ctx, "find spawn examples", "https://docs.rs/tokio/", &body).await;
    let recorded = record.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].starts_with("model=haiku-test|user="));
}

#[tokio::test]
async fn refiner_output_includes_raw_path_and_size() {
    let resolver = Arc::new(MockResolver {
        model: Some("haiku-test".into()),
        response: Some(Ok("summary".into())),
        ..Default::default()
    });
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(20_000);
    let r = try_refine(&ctx, "intent", "https://example.com/", &body)
        .await
        .expect("refine should fire");
    assert!(r.content.contains("raw_path: "));
    assert!(r.content.contains("raw_size: "));
    assert!(r.content.contains("KB"));
}

#[test]
fn settings_default_threshold_is_8kb() {
    let cfg = loopal_config::FetchRefinerConfig::default();
    assert_eq!(cfg.threshold_bytes, 8 * 1024);
    assert!(cfg.enabled);
}

#[test]
fn settings_does_not_pin_a_specific_model() {
    let cfg = loopal_config::FetchRefinerConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    assert!(
        !json.contains("model"),
        "model should be sourced from model_routing[refine], not embedded — got {json}"
    );
}

#[test]
fn one_shot_chat_error_displays_distinct_messages() {
    let messages: Vec<String> = [
        OneShotChatError::Timeout,
        OneShotChatError::ProviderUnresolvable,
        OneShotChatError::StreamFailed,
        OneShotChatError::ChunkFailed,
        OneShotChatError::EmptyResponse,
    ]
    .iter()
    .map(|e| e.to_string())
    .collect();
    let unique: std::collections::HashSet<&String> = messages.iter().collect();
    assert_eq!(
        unique.len(),
        messages.len(),
        "every variant must be distinguishable"
    );
}

#[tokio::test]
async fn refiner_does_not_call_chat_when_model_is_none() {
    let resolver = Arc::new(MockResolver {
        model: None,
        response: Some(Ok("never".into())),
        ..Default::default()
    });
    let record = resolver.record.clone();
    let (ctx, _tmp) = make_ctx_with_resolver(Some(resolver));
    let body = big_body(20_000);
    let _ = try_refine(&ctx, "intent", "https://example.com/", &body).await;
    assert_eq!(
        record.lock().unwrap().len(),
        0,
        "no model → chat must not fire"
    );
}
