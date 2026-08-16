use super::*;

#[tokio::test]
async fn incomplete_server_calls_and_pause_turn_request_continuations() {
    let mut fixture = started_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 2);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            server_blocks: vec![ContentBlock::ServerToolUse {
                id: "orphan-search".into(),
                name: "web_search".into(),
                input: serde_json::json!({"query": "coverage"}),
            }],
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(
        state,
        TurnState::NeedsContinuation {
            reason: ContinuationReason::StreamTruncated
        }
    ));

    let mut fixture = started_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            assistant_text: "server pause".into(),
            stop_reason: StopReason::PauseTurn,
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(
        state,
        TurnState::NeedsContinuation {
            reason: ContinuationReason::PauseTurn
        }
    ));
}

#[tokio::test]
async fn cancellation_and_empty_stream_errors_finish_without_continuation() {
    let mut fixture = started_fixture();
    fixture.runner.interrupt.signal();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            assistant_text: "cancelled partial".into(),
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(state, TurnState::Complete));

    let mut fixture = make_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            stream_error: true,
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(state, TurnState::Complete));
}

#[tokio::test]
async fn max_tokens_and_tool_calls_select_their_distinct_next_states() {
    let mut fixture = make_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            stop_reason: StopReason::MaxTokens,
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(
        state,
        TurnState::NeedsContinuation {
            reason: ContinuationReason::MaxTokensWithoutTools
        }
    ));

    let mut fixture = make_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(1, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            stop_reason: StopReason::MaxTokens,
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(state, TurnState::Complete));

    let mut fixture = started_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            assistant_text: "using a tool".into(),
            tool_uses: vec![(
                "tool-1".into(),
                "Read".into(),
                serde_json::json!({"path": "README.md"}),
            )],
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(
        state,
        TurnState::NeedsToolExecution { tool_uses } if tool_uses.len() == 1
    ));
}

#[tokio::test]
async fn thinking_only_stream_failure_continues_but_exhausted_pause_completes() {
    let mut fixture = started_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            thinking_text: "unsigned partial thought".into(),
            stream_error: true,
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(
        state,
        TurnState::NeedsContinuation {
            reason: ContinuationReason::StreamTruncated
        }
    ));

    let mut fixture = started_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(1, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            assistant_text: "pause budget exhausted".into(),
            stop_reason: StopReason::PauseTurn,
            ..Default::default()
        },
        &mut counters,
    )
    .await
    .unwrap();
    assert!(matches!(state, TurnState::Complete));
}
