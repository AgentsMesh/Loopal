use std::sync::Arc;

use loopal_config::{HookConfig, HookEvent, Settings};
use loopal_error::LoopalError;
use loopal_protocol::event_id::scope_turn;
use loopal_provider_api::{ContentBlock, ContinuationReason, StopReason};
use loopal_turn::TurnTrigger;

use super::support::{Fixture, make_fixture};
use crate::agent_loop::cancel::TurnCancel;
use crate::agent_loop::llm_result::LlmStreamResult;
use crate::agent_loop::turn_context::TurnContext;
use crate::agent_loop::turn_response::TurnLoopCounters;
use crate::agent_loop::turn_state::TurnState;

mod states;

fn make_turn_context(fixture: &Fixture) -> TurnContext {
    TurnContext::new(
        1,
        TurnCancel::new(
            fixture.runner.interrupt.clone(),
            fixture.runner.interrupt_tx.clone(),
        ),
    )
}

fn make_counters(continuations: u32, max_continuations: u32) -> TurnLoopCounters {
    TurnLoopCounters {
        continuation_count: continuations,
        stop_feedback_count: 0,
        max_continuations,
        max_stop_feedback: 0,
    }
}

fn started_fixture() -> Fixture {
    let mut fixture = make_fixture();
    fixture
        .runner
        .start_turn_record(TurnTrigger::Resume)
        .unwrap();
    fixture
}

async fn handle(
    fixture: &mut Fixture,
    turn_context: &mut TurnContext,
    result: LlmStreamResult,
    counters: &mut TurnLoopCounters,
) -> loopal_error::Result<TurnState> {
    scope_turn(
        turn_context.turn_id,
        fixture
            .runner
            .handle_response_recorded(turn_context, result, counters),
    )
    .await
}

#[tokio::test]
async fn terminal_provider_errors_preserve_partial_text_and_empty_output() {
    for assistant_text in ["partial answer", ""] {
        let mut fixture = make_fixture();
        let mut turn_context = make_turn_context(&fixture);
        let mut counters = make_counters(0, 1);
        let result = LlmStreamResult {
            assistant_text: assistant_text.into(),
            terminal_error: Some(LoopalError::Other("terminal provider failure".into())),
            ..Default::default()
        };

        let error = match handle(&mut fixture, &mut turn_context, result, &mut counters).await {
            Err(error) => error,
            Ok(_) => panic!("terminal provider error unexpectedly succeeded"),
        };

        assert_eq!(error.to_string(), "terminal provider failure");
        assert_eq!(turn_context.best_effort_output(), assistant_text);
    }
}

#[tokio::test]
async fn truncated_streams_continue_then_return_the_authoritative_failure() {
    let mut fixture = started_fixture();
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 1);
    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult {
            assistant_text: "unterminated".into(),
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
    assert_eq!(counters.continuation_count, 1);

    for failure in [
        Some(LoopalError::Other("original stream failure".into())),
        None,
    ] {
        let mut fixture = started_fixture();
        let mut turn_context = make_turn_context(&fixture);
        let mut counters = make_counters(1, 1);
        let outcome = handle(
            &mut fixture,
            &mut turn_context,
            LlmStreamResult {
                assistant_text: "still unterminated".into(),
                stream_error: true,
                stream_failure: failure,
                ..Default::default()
            },
            &mut counters,
        )
        .await;
        let error = match outcome {
            Err(error) => error,
            Ok(_) => panic!("exhausted truncated stream unexpectedly succeeded"),
        };
        assert!(
            error.to_string().to_ascii_lowercase().contains("stream"),
            "unexpected stream failure: {error}"
        );
    }
}

#[tokio::test]
async fn stop_hook_feedback_requests_another_model_round() {
    let mut fixture = started_fixture();
    fixture.runner.params.deps.kernel = Arc::new(
        loopal_kernel::Kernel::new(Settings {
            hooks: vec![HookConfig {
                event: HookEvent::Stop,
                command: "printf '%s' '{\"additional_context\":\"address the review\"}'".into(),
                tool_filter: None,
                timeout_ms: 5_000,
                hook_type: Default::default(),
                url: None,
                headers: Default::default(),
                prompt: None,
                model: None,
                condition: None,
                id: None,
            }],
            ..Default::default()
        })
        .unwrap(),
    );
    let mut turn_context = make_turn_context(&fixture);
    let mut counters = make_counters(0, 0);
    counters.max_stop_feedback = 1;

    let state = handle(
        &mut fixture,
        &mut turn_context,
        LlmStreamResult::default(),
        &mut counters,
    )
    .await
    .unwrap();

    assert!(matches!(
        state,
        TurnState::NeedsStopFeedback { feedback } if feedback == "address the review"
    ));
    assert_eq!(counters.stop_feedback_count, 1);
}
