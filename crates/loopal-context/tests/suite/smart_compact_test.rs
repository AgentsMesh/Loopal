use async_trait::async_trait;
use loopal_context::middleware::smart_compact::{
    CompactRetryEvent, CompactRetryObserver, compact_to_boundary, compact_to_boundary_observed,
};
use loopal_context::middleware::touched_files::rank_touched_files;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{Turn, TurnTrigger};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

fn user_turn(content: &str) -> Turn {
    Turn::new(TurnTrigger::UserInput {
        envelope_id: content.into(),
        content: content.into(),
        images: Vec::new(),
    })
}

// `compact_to_boundary` requires a live Provider, so its happy-path is exercised
// in runtime integration tests. Here we cover the deterministic helpers it uses.

fn successful_tool_exchange(name: &str, path: &str) -> Vec<Message> {
    let id = format!("{name}-{path}");
    vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: id.clone(),
                name: name.to_string(),
                input: serde_json::json!({ "file_path": path }),
            }],
            origin: None,
            ephemeral_in_history: false,
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: id,
                content: "ok".into(),
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }],
            origin: None,
            ephemeral_in_history: false,
        },
    ]
}

#[test]
fn touched_files_includes_only_file_tools() {
    let messages: Vec<_> = [
        successful_tool_exchange("Read", "/a.rs"),
        successful_tool_exchange("Bash", "/ignored"),
        successful_tool_exchange("Write", "/b.rs"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let files = rank_touched_files(&messages, 10);
    let paths: Vec<&str> = files.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"/b.rs"));
    assert!(paths.contains(&"/a.rs"));
}

#[test]
fn touched_files_mutations_promoted_to_top() {
    let messages: Vec<_> = [
        successful_tool_exchange("Read", "/x.rs"),
        successful_tool_exchange("Read", "/y.rs"),
        successful_tool_exchange("Write", "/z.rs"),
    ]
    .into_iter()
    .flatten()
    .collect();
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
            retry_after_ms: None,
        }))
    }
}

struct PendingHandshakeProvider;

enum CompactAttempt {
    Chunks(Vec<Result<StreamChunk, LoopalError>>),
    Error(LoopalError),
    PendingStream,
}

struct ScriptedCompactProvider {
    attempts: Mutex<VecDeque<CompactAttempt>>,
    calls: AtomicUsize,
}

impl ScriptedCompactProvider {
    fn new(attempts: Vec<CompactAttempt>) -> Self {
        Self {
            attempts: Mutex::new(attempts.into()),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Provider for ScriptedCompactProvider {
    fn name(&self) -> &str {
        "scripted-compact"
    }

    async fn stream_chat(&self, _params: &ChatParams) -> Result<ChatStream, LoopalError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self
            .attempts
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted compaction attempt")
        {
            CompactAttempt::Chunks(chunks) => Ok(Box::pin(futures::stream::iter(chunks))),
            CompactAttempt::Error(error) => Err(error),
            CompactAttempt::PendingStream => Ok(Box::pin(futures::stream::pending())),
        }
    }
}

fn compact_done(text: &str) -> CompactAttempt {
    CompactAttempt::Chunks(vec![
        Ok(StreamChunk::Text { text: text.into() }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ])
}

#[derive(Default)]
struct RecordingRetryObserver {
    events: Mutex<Vec<CompactRetryEvent>>,
}

#[async_trait]
impl CompactRetryObserver for RecordingRetryObserver {
    async fn observe(&self, event: CompactRetryEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl RecordingRetryObserver {
    fn into_events(self) -> Vec<CompactRetryEvent> {
        self.events.into_inner().unwrap()
    }
}

#[async_trait]
impl Provider for PendingHandshakeProvider {
    fn name(&self) -> &str {
        "pending-handshake"
    }

    async fn stream_chat(&self, _params: &ChatParams) -> Result<ChatStream, LoopalError> {
        std::future::pending::<Result<ChatStream, LoopalError>>().await
    }
}

#[tokio::test]
async fn cancel_token_wakes_compact_retry_sleep_promptly() {
    let provider = AlwaysRetryableErrProvider;
    let turns = vec![user_turn("hello"), user_turn("world")];
    let cancel = CancellationToken::new();
    cancel.cancel();

    let start = Instant::now();
    let result = compact_to_boundary(&turns, &provider, "claude-haiku-4-5", 2, None, &cancel).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(800),
        "cancel must short-circuit RETRY_BACKOFF (1s+2s+4s); elapsed={elapsed:?}",
    );
    assert!(
        result.is_err(),
        "cancellation must abort compaction instead of rewriting history"
    );
}

#[tokio::test]
async fn cancel_after_first_retry_still_short_circuits() {
    let provider = AlwaysRetryableErrProvider;
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_clone.cancel();
    });

    let start = Instant::now();
    let result = compact_to_boundary(&turns, &provider, "claude-haiku-4-5", 2, None, &cancel).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(900),
        "mid-retry cancel must abort the next backoff sleep; elapsed={elapsed:?}",
    );
    assert!(result.is_err(), "cancel must not produce a summary");
}

#[tokio::test]
async fn cancel_interrupts_provider_handshake_and_reports_terminal_retry_state() {
    let provider = PendingHandshakeProvider;
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();
    let events = RecordingRetryObserver::default();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_clone.cancel();
    });

    let start = Instant::now();
    let result = compact_to_boundary_observed(
        &turns,
        &provider,
        "claude-haiku-4-5",
        2,
        None,
        &cancel,
        &events,
    )
    .await;

    assert!(start.elapsed() < Duration::from_millis(800));
    assert!(result.is_err(), "cancel must not produce a summary");
    assert_eq!(
        events.into_events(),
        vec![CompactRetryEvent::Cancelled { retries: 0 }]
    );
}

#[tokio::test]
async fn retry_observer_has_balanced_start_and_cancel_events() {
    let provider = AlwaysRetryableErrProvider;
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();
    let events = RecordingRetryObserver::default();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_clone.cancel();
    });

    let result = compact_to_boundary_observed(
        &turns,
        &provider,
        "claude-haiku-4-5",
        2,
        None,
        &cancel,
        &events,
    )
    .await;

    assert!(result.is_err(), "cancel must not produce a summary");

    let events = events.into_events();
    assert!(matches!(
        events.first(),
        Some(CompactRetryEvent::Scheduled {
            attempt: 1,
            max_retries: 3,
            wait,
            ..
        }) if *wait == Duration::from_secs(1)
    ));
    assert_eq!(
        events.last(),
        Some(&CompactRetryEvent::Cancelled { retries: 1 })
    );
}

#[tokio::test(start_paused = true)]
async fn compact_retries_partial_eof_instead_of_accepting_truncated_summary() {
    let provider = ScriptedCompactProvider::new(vec![
        CompactAttempt::Chunks(vec![Ok(StreamChunk::Text {
            text: "<summary>truncated".into(),
        })]),
        compact_done("<summary>complete summary</summary>"),
    ]);
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();
    let events = RecordingRetryObserver::default();

    let output = compact_to_boundary_observed(
        &turns,
        &provider,
        "claude-haiku-4-5",
        2,
        None,
        &cancel,
        &events,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(provider.calls(), 2);
    assert!(
        output
            .summary_msg
            .text_content()
            .contains("complete summary")
    );
    assert!(!output.summary_msg.text_content().contains("truncated"));
    assert!(matches!(
        events.into_events().as_slice(),
        [
            CompactRetryEvent::Scheduled { .. },
            CompactRetryEvent::Succeeded { retries: 1 }
        ]
    ));
}

#[tokio::test(start_paused = true)]
async fn compact_idle_stream_times_out_and_retries() {
    let provider = ScriptedCompactProvider::new(vec![
        CompactAttempt::PendingStream,
        compact_done("<summary>after idle</summary>"),
    ]);
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();

    let output = compact_to_boundary(&turns, &provider, "model", 2, None, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(provider.calls(), 2);
    assert!(output.summary_msg.text_content().contains("after idle"));
}

#[tokio::test(start_paused = true)]
async fn compact_honors_structured_retry_after_delay() {
    let provider = ScriptedCompactProvider::new(vec![
        CompactAttempt::Error(LoopalError::Provider(ProviderError::Api {
            status: 503,
            message: "busy".into(),
            retry_after_ms: Some(17),
        })),
        compact_done("<summary>after retry-after</summary>"),
    ]);
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();
    let events = RecordingRetryObserver::default();

    compact_to_boundary_observed(&turns, &provider, "model", 2, None, &cancel, &events)
        .await
        .unwrap();

    assert!(matches!(
        events.into_events().first(),
        Some(CompactRetryEvent::Scheduled { wait, .. }) if *wait == Duration::from_millis(17)
    ));
}

#[tokio::test(start_paused = true)]
async fn compact_eof_exhaustion_uses_deterministic_fallback() {
    let provider =
        ScriptedCompactProvider::new((0..4).map(|_| CompactAttempt::Chunks(Vec::new())).collect());
    let turns = vec![user_turn("a"), user_turn("b")];
    let cancel = CancellationToken::new();
    let events = RecordingRetryObserver::default();

    let output =
        compact_to_boundary_observed(&turns, &provider, "model", 2, None, &cancel, &events)
            .await
            .unwrap()
            .unwrap();

    assert_eq!(provider.calls(), 4);
    assert!(output.summary_msg.text_content().contains("Bare Summary"));
    assert_eq!(
        events.into_events().last(),
        Some(&CompactRetryEvent::Exhausted { retries: 3 })
    );
}
