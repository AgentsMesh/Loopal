//! Shared async waiters for `AgentEvent` variants used by end-to-end tests.
//!
//! **Scope guard**: this module is restricted to *async polling/blocking
//! waiters for AgentEvent (or LLM call) variants*. Anything else — fixture
//! builders, mock factories, assertion macros — goes in a dedicated module.
//! The point is to keep this from drifting into a `helpers/utils` dumping
//! ground.

use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ContinuationGateSummary, DegenerationSummary,
};

pub(crate) async fn wait_for_degeneration_event(
    rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>,
) -> DegenerationSummary {
    loop {
        let ev = rx.recv().await.expect("event channel closed");
        if let AgentEventPayload::DegenerationDetected(s) = ev.payload {
            return s;
        }
    }
}

pub(crate) async fn wait_for_gate_change(
    rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>,
    want_open: bool,
) -> ContinuationGateSummary {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for ContinuationGateChanged");
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(ev)) => {
                if let AgentEventPayload::ContinuationGateChanged(s) = ev.payload
                    && s.open == want_open
                {
                    return s;
                }
            }
            Ok(None) => panic!("event channel closed"),
            Err(_) => panic!("timed out waiting for ContinuationGateChanged"),
        }
    }
}

pub(crate) async fn wait_for_running_event(rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>) {
    wait_for_event_variant(rx, "Running", |p| matches!(p, AgentEventPayload::Running)).await;
}

pub(crate) async fn wait_for_interrupted_event(rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>) {
    wait_for_event_variant(rx, "Interrupted", |p| {
        matches!(p, AgentEventPayload::Interrupted)
    })
    .await;
}

/// Wait until the runner emits its first `Stream` chunk for the current
/// turn. Used to synchronise tests with "LLM is actively streaming" — more
/// reliable than `tokio::time::sleep` because it pins on actual progress
/// rather than guessing at chunk pacing.
pub(crate) async fn wait_for_stream_event(rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>) {
    wait_for_event_variant(rx, "Stream", |p| {
        matches!(p, AgentEventPayload::Stream { .. })
    })
    .await;
}

async fn wait_for_event_variant(
    rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>,
    label: &str,
    pred: impl Fn(&AgentEventPayload) -> bool,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for {label} event");
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(ev)) => {
                if pred(&ev.payload) {
                    return;
                }
            }
            Ok(None) => panic!("event channel closed"),
            Err(_) => panic!("timed out waiting for {label}"),
        }
    }
}

pub(crate) async fn wait_for_tool_error(
    handle: &Arc<std::sync::Mutex<Vec<Vec<loopal_turn::Turn>>>>,
) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = handle.lock().unwrap().clone();
        for batch in snapshot.iter().rev() {
            for turn in batch.iter().rev() {
                for step in turn.body.steps.iter().rev() {
                    let loopal_turn::TurnStep::ToolBatch(b) = step else {
                        continue;
                    };
                    for item in &b.items {
                        if let loopal_turn::ToolExecState::Done(r) = &item.state
                            && r.is_error
                        {
                            return r.content.clone();
                        }
                    }
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for tool_result");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub(crate) async fn wait_for_call_count(
    handle: &Arc<std::sync::Mutex<Vec<Vec<loopal_turn::Turn>>>>,
    target: usize,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let actual = handle.lock().unwrap().len();
        if actual >= target {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            let tails: Vec<String> = handle
                .lock()
                .unwrap()
                .iter()
                .filter_map(|batch| batch.last().map(turn_text_summary))
                .map(|t| t.chars().take(40).collect::<String>())
                .collect();
            panic!(
                "timed out waiting for {target} LLM calls (saw {actual}); per-call last-message tails: {tails:?}"
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub(crate) async fn wait_for_recorded_text(
    handle: &Arc<std::sync::Mutex<Vec<Vec<loopal_turn::Turn>>>>,
    needle: &str,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let snapshot = handle.lock().unwrap().clone();
        for batch in snapshot.iter().rev() {
            for t in batch.iter().rev() {
                if turn_text_summary(t).contains(needle) {
                    return;
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for text {needle:?} in recorded turns");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// reason: tests use this to inspect the trigger content and any assistant
/// text-block output of a Turn. Mirrors the old `Message::text_content` flat
/// view but adapted to Turn's split trigger/steps shape.
fn turn_text_summary(turn: &loopal_turn::Turn) -> String {
    use loopal_turn::{TurnStep, TurnTrigger};
    let mut out = match &turn.trigger {
        TurnTrigger::UserInput { content, .. }
        | TurnTrigger::Cron { content, .. }
        | TurnTrigger::Agent { content, .. }
        | TurnTrigger::Channel { content, .. }
        | TurnTrigger::GoalContinuation { content, .. }
        | TurnTrigger::BackgroundHook { content, .. } => content.clone(),
        TurnTrigger::Resume => String::new(),
    };
    for step in &turn.body.steps {
        if let TurnStep::LlmCall { response, .. } = step {
            for tb in &response.text_blocks {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&tb.text);
            }
        }
    }
    out
}
