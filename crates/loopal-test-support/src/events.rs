//! Event collection with timeout and payload extraction helpers.

use std::time::Duration;

use tokio::sync::mpsc;

use loopal_protocol::{AgentEvent, AgentEventPayload};

/// Default timeout for event collection (10 seconds).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Collect events until `AwaitingInput` or `Finished`, with timeout.
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
