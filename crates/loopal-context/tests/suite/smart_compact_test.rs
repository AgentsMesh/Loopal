use async_trait::async_trait;
use loopal_context::middleware::smart_compact::compact_to_boundary;
use loopal_context::middleware::touched_files::rank_touched_files;
use loopal_error::{LoopalError, ProviderError};
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider_api::{ChatParams, ChatStream, Provider};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

// `compact_to_boundary` requires a live Provider, so its happy-path is exercised
// in runtime integration tests. Here we cover the deterministic helpers it uses.

fn tool_use(name: &str, path: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: format!("{name}-{path}"),
            name: name.to_string(),
            input: serde_json::json!({ "file_path": path }),
        }],
        origin: None,
        ephemeral_in_history: false,
    }
}

#[test]
fn touched_files_includes_only_file_tools() {
    let messages = vec![
        tool_use("Read", "/a.rs"),
        tool_use("Bash", "/ignored"),
        tool_use("Write", "/b.rs"),
    ];
    let files = rank_touched_files(&messages, 10);
    let paths: Vec<&str> = files.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"/b.rs"));
    assert!(paths.contains(&"/a.rs"));
}

#[test]
fn touched_files_mutations_promoted_to_top() {
    let messages = vec![
        tool_use("Read", "/x.rs"),
        tool_use("Read", "/y.rs"),
        tool_use("Write", "/z.rs"),
    ];
    let files = rank_touched_files(&messages, 10);
    assert_eq!(files[0].path, "/z.rs");
    assert!(files[0].mutated);
}

struct AlwaysRetryableErrProvider;

#[async_trait]
impl Provider for AlwaysRetryableErrProvider {
    fn name(&self) -> &str {
        "always-retryable-err"
    }
    async fn stream_chat(&self, _params: &ChatParams) -> Result<ChatStream, LoopalError> {
        // 5xx -> retryable, exercises the retry/sleep loop without consuming network.
        Err(LoopalError::Provider(ProviderError::Api {
            status: 503,
            message: "simulated".into(),
        }))
    }
}

#[tokio::test]
async fn cancel_token_wakes_compact_retry_sleep_promptly() {
    let provider = AlwaysRetryableErrProvider;
    let messages = vec![Message::user("hello"), Message::user("world")];
    let cancel = CancellationToken::new();
    cancel.cancel();

    let start = Instant::now();
    let result = compact_to_boundary(&messages, &provider, "claude-haiku-4-5", 2, None, &cancel)
        .await
        .expect("compact_to_boundary always falls back to bare_summary on LLM failure");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(800),
        "cancel must short-circuit RETRY_BACKOFF (1s+2s+4s); elapsed={elapsed:?}",
    );
    assert!(
        result.is_some(),
        "fallback summary should still be produced"
    );
    let out = result.unwrap();
    assert!(
        out.summary_msg.text_content().contains("Bare Summary"),
        "cancel path must surface bare_summary fallback",
    );
}

#[tokio::test]
async fn cancel_after_first_retry_still_short_circuits() {
    let provider = AlwaysRetryableErrProvider;
    let messages = vec![Message::user("a"), Message::user("b")];
    let cancel = CancellationToken::new();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_clone.cancel();
    });

    let start = Instant::now();
    let result = compact_to_boundary(&messages, &provider, "claude-haiku-4-5", 2, None, &cancel)
        .await
        .expect("bare_summary fallback");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(900),
        "mid-retry cancel must abort the next backoff sleep; elapsed={elapsed:?}",
    );
    assert!(result.is_some());
}
