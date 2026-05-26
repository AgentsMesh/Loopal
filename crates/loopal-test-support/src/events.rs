//! Event collection with timeout and payload extraction helpers.

use std::time::Duration;

use tokio::sync::mpsc;

use loopal_protocol::{AgentEvent, AgentEventPayload};

/// Default timeout for event collection (10 seconds).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Collect events until `AwaitingInput` or `Finished`, with timeout.
///
/// `AwaitingInput` IS terminal here — used for Persistent-lifecycle tests
/// where the agent goes idle between turns. Ephemeral-lifecycle tests with
/// headless prompts should use [`collect_until_finished`] instead, since
/// they emit a transient `AwaitingInput` before draining the queued
/// envelope.
///
/// Calls `observer` for each event before storing it (e.g., to feed a
/// `SessionController`). Panics on timeout.
pub async fn collect_until_idle(
    rx: &mut mpsc::Receiver<AgentEvent>,
    timeout: Duration,
    mut observer: impl FnMut(&AgentEvent),
) -> Vec<AgentEventPayload> {
    let mut collected = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(event)) => {
                let is_terminal = matches!(
                    &event.payload,
                    AgentEventPayload::AwaitingInput | AgentEventPayload::Finished
                );
                observer(&event);
                collected.push(event.payload);
                if is_terminal {
                    break;
                }
            }
            Ok(None) => break, // channel closed
            Err(_) => panic!(
                "collect_until_idle timed out after {timeout:?} — collected {} events",
                collected.len()
            ),
        }
    }
    collected
}

/// Collect events until `Finished`. Use for Ephemeral-lifecycle agents
/// whose `AwaitingInput` may be transient (emitted by the idle phase
/// before draining a queued envelope, e.g. headless `--prompt`).
pub async fn collect_until_finished(
    rx: &mut mpsc::Receiver<AgentEvent>,
    timeout: Duration,
) -> Vec<AgentEventPayload> {
    let mut collected = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(event)) => {
                let is_finished = matches!(&event.payload, AgentEventPayload::Finished);
                collected.push(event.payload);
                if is_finished {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => panic!(
                "collect_until_finished timed out after {timeout:?} — collected {} events",
                collected.len()
            ),
        }
    }
    collected
}

/// Collect events until `predicate` returns true on the most recent event.
/// Use when waiting on a single control-induced emission (e.g. `Cleared`)
/// — these never push the runner through a Running → AwaitingInput
/// terminal, so `collect_until_idle` would block forever.
pub async fn collect_until<P>(
    rx: &mut mpsc::Receiver<AgentEvent>,
    timeout: Duration,
    mut predicate: P,
    mut observer: impl FnMut(&AgentEvent),
) -> Vec<AgentEventPayload>
where
    P: FnMut(&AgentEventPayload) -> bool,
{
    let mut collected = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(event)) => {
                let matched = predicate(&event.payload);
                observer(&event);
                collected.push(event.payload);
                if matched {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => panic!(
                "collect_until timed out after {timeout:?} — collected {} events",
                collected.len()
            ),
        }
    }
    collected
}

/// Concatenate all `Stream { text }` payloads into a single string.
pub fn extract_texts(events: &[AgentEventPayload]) -> String {
    let mut out = String::new();
    for e in events {
        if let AgentEventPayload::Stream { text } = e {
            out.push_str(text);
        }
    }
    out
}

/// Extract all `ToolCall` names in order.
pub fn extract_tool_names(events: &[AgentEventPayload]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| {
            if let AgentEventPayload::ToolCall { name, .. } = e {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Extract all ToolResult (name, is_error) pairs in order.
pub fn extract_tool_results(events: &[AgentEventPayload]) -> Vec<(String, bool)> {
    events
        .iter()
        .filter_map(|e| {
            if let AgentEventPayload::ToolResult { name, is_error, .. } = e {
                Some((name.clone(), *is_error))
            } else {
                None
            }
        })
        .collect()
}

/// Non-blocking drain of currently-pending events. Yields once to let
/// any in-flight emit complete, then flushes the channel via try_recv.
/// Use this after a synchronous `runner.method().await` to inspect what
/// events that single call produced — `collect_until_idle` waits for a
/// terminal event which compact-internal methods don't emit.
pub async fn drain_pending(rx: &mut mpsc::Receiver<AgentEvent>) -> Vec<AgentEventPayload> {
    tokio::task::yield_now().await;
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        out.push(ev.payload);
    }
    out
}
