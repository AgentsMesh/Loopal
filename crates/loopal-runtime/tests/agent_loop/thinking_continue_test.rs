//! Tests for thinking-mode auto-continuation and stop-feedback message injection.

use loopal_message::MessageRole;
use loopal_provider_api::{StopReason, StreamChunk, ThinkingConfig};
use loopal_runtime::AgentConfig;
use loopal_tool_api::PermissionMode;

use super::mock_provider::{make_multi_runner, make_multi_runner_with_config};

/// Default model (`claude-sonnet-4-20250514`) has `BudgetRequired` thinking
/// capability, and default `ThinkingConfig::Auto` resolves to `Some(Budget{..})`.
/// After MaxTokens auto-continuation, a synthetic user message must be injected.
#[tokio::test]
async fn test_thinking_active_injects_continuation_on_max_tokens() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "part 1".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "part 2".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    // Verify: between the two assistant messages there should be a user
    // message containing the continuation prompt.
    let messages = runner.params.store.messages();
    let has_continuation = messages.iter().any(|m| {
        m.role == MessageRole::User
            && m.content.iter().any(|b| match b {
                loopal_message::ContentBlock::Text { text } => text.contains("[Continue"),
                _ => false,
            })
    });
    assert!(
        has_continuation,
        "thinking-active auto-continuation must inject a user message"
    );
}

/// With thinking explicitly disabled, no synthetic user message should be
/// injected — the model continues via assistant prefill.
#[tokio::test]
async fn test_thinking_disabled_no_continuation_message() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "part 1".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "part 2".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let config = AgentConfig {
        thinking_config: ThinkingConfig::Disabled,
        permission_mode: PermissionMode::Bypass,
        ..Default::default()
    };
    let (mut runner, mut event_rx) = make_multi_runner_with_config(calls, config);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let messages = runner.params.store.messages();
    let has_continuation = messages.iter().any(|m| {
        m.role == MessageRole::User
            && m.content.iter().any(|b| match b {
                loopal_message::ContentBlock::Text { text } => text.contains("[Continue"),
                _ => false,
            })
    });
    assert!(
        !has_continuation,
        "thinking-disabled must NOT inject continuation messages"
    );
}

/// MaxTokens with truncated tool calls + thinking active → continuation injected.
#[tokio::test]
async fn test_thinking_truncated_tools_injects_continuation() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "Let me ".into(),
            }),
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/truncated"}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "read the file.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let messages = runner.params.store.messages();
    let has_continuation = messages.iter().any(|m| {
        m.role == MessageRole::User
            && m.content.iter().any(|b| match b {
                loopal_message::ContentBlock::Text { text } => text.contains("[Continue"),
                _ => false,
            })
    });
    assert!(
        has_continuation,
        "truncated tools + thinking must inject continuation"
    );
}

/// After recording an assistant message, stop-hook feedback must be stored
/// as a new User message (not appended to the assistant message).
#[tokio::test]
async fn test_messages_alternate_roles_after_continuation() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "part 1".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "part 2".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    // Verify strict role alternation (ignoring system messages at the front).
    let messages = runner.params.store.messages();
    let non_system: Vec<_> = messages
        .iter()
        .filter(|m| m.role != MessageRole::System)
        .collect();
    for pair in non_system.windows(2) {
        assert_ne!(
            pair[0].role, pair[1].role,
            "consecutive messages must alternate roles: {:?} followed by {:?}",
            pair[0].role, pair[1].role
        );
    }
}
