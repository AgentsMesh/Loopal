use std::time::Instant;

use loopal_tool_invocation::{
    FailureKind, InvocationId, Outcome, ProgressSnapshot, StaleReason, ToolInvocation,
    ToolResultMetadata, TransitionCmd, transition,
};
use serde_json::Value;
use tracing::warn;

use super::agent_conversation::AgentConversation;
use super::truncate::{truncate_json, truncate_result_for_storage};
use super::types::SessionMessage;

pub(crate) fn handle_tool_call(
    conv: &mut AgentConversation,
    id: String,
    name: String,
    input: Value,
) -> bool {
    let Ok(invocation_id) = InvocationId::new(id) else {
        warn!("ignoring tool_call with empty id");
        return false;
    };
    if conv
        .messages
        .iter()
        .any(|m| m.tool_calls.iter().any(|tc| tc.id == invocation_id))
    {
        warn!(id = %invocation_id, "tool_call duplicate id rejected");
        return false;
    }
    conv.flush_streaming();
    let summary = format!("{}({})", name, truncate_json(&input, 60));
    let invocation =
        ToolInvocation::start(invocation_id, name, summary, Some(input), Instant::now());
    if let Some(last) = conv.messages.last_mut()
        && last.role == "assistant"
    {
        last.tool_calls.push(invocation);
        return true;
    }
    conv.messages.push(SessionMessage {
        role: "assistant".to_string(),
        tool_calls: vec![invocation],
        ..Default::default()
    });
    true
}

pub(crate) struct ToolResultParams {
    pub id: String,
    pub name: String,
    pub result: String,
    pub is_error: bool,
    pub metadata: Option<ToolResultMetadata>,
}

pub(crate) fn handle_tool_result(conv: &mut AgentConversation, p: ToolResultParams) -> bool {
    let Ok(target_id) = InvocationId::new(p.id.clone()) else {
        warn!(tool = %p.name, "tool_result with empty id rejected");
        return false;
    };
    let now = Instant::now();
    for msg in conv.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.id == target_id {
                let prev = tc.clone();
                let cmd = build_terminal_cmd(&p);
                match transition(prev, cmd, now) {
                    Ok(next) => {
                        tc.state = next.state;
                        tc.metadata = p.metadata;
                        return true;
                    }
                    Err(e) => {
                        warn!(id = %target_id, error = %e, "tool_result transition failed");
                        return false;
                    }
                }
            }
        }
    }
    warn!(id = %target_id, "tool_result for unknown invocation");
    false
}

fn build_terminal_cmd(p: &ToolResultParams) -> TransitionCmd {
    match &p.metadata {
        Some(ToolResultMetadata::Stale { reason }) => TransitionCmd::MarkStale(*reason),
        Some(ToolResultMetadata::Cancelled { cause }) => TransitionCmd::Cancel(*cause),
        Some(
            ToolResultMetadata::BytesWritten { .. } | ToolResultMetadata::ModifiedFiles { .. },
        )
        | None => {
            let outcome = if p.is_error {
                Outcome::Failure {
                    error: truncate_result_for_storage(&p.result),
                    kind: FailureKind::ToolError,
                }
            } else {
                Outcome::Success {
                    content: truncate_result_for_storage(&p.result),
                }
            };
            TransitionCmd::Complete(outcome)
        }
    }
}

pub(crate) fn handle_tool_batch_start(conv: &mut AgentConversation, tool_ids: &[String]) {
    let batch_id = format!("batch-{}", conv.turn_count);
    for msg in conv.messages.iter_mut().rev() {
        if msg.role != "assistant" || msg.tool_calls.is_empty() {
            continue;
        }
        let mut found = false;
        for tc in msg.tool_calls.iter_mut() {
            if tc.state.is_active() && tool_ids.iter().any(|s| s == tc.id.as_str()) {
                tc.batch_id = Some(batch_id.clone());
                found = true;
            }
        }
        if found {
            break;
        }
    }
}

pub(crate) fn handle_tool_progress(conv: &mut AgentConversation, id: String, output_tail: String) {
    let Ok(target_id) = InvocationId::new(id) else {
        warn!("tool_progress with empty id rejected");
        return;
    };
    let now = Instant::now();
    for msg in conv.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.id == target_id {
                if tc.state.is_terminal() {
                    return;
                }
                let prev = tc.clone();
                let snap = ProgressSnapshot::new(output_tail.clone());
                match transition(prev, TransitionCmd::RecordProgress(snap), now) {
                    Ok(next) => tc.state = next.state,
                    Err(e) => warn!(id = %target_id, error = %e, "tool_progress transition failed"),
                }
                return;
            }
        }
    }
    tracing::debug!(id = %target_id, "tool_progress for unknown invocation (likely race)");
}

pub(crate) fn handle_turn_end_reconcile(conv: &mut AgentConversation) -> usize {
    let now = Instant::now();
    let mut reconciled = 0usize;
    for msg in conv.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.state.is_active() {
                let prev = tc.clone();
                if let Ok(next) =
                    transition(prev, TransitionCmd::MarkStale(StaleReason::TurnEnded), now)
                {
                    tc.state = next.state;
                    reconciled += 1;
                }
            }
        }
    }
    reconciled
}
