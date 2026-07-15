use loopal_protocol::{AgentStatus, ContinuationGateSummary, GateCloseReason};

use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn cleared(state: &mut SessionViewState, context_window: u32) -> MutationEffect {
    state.agent.conversation.clear_all(context_window);
    let obs = &mut state.agent.observable;
    obs.input_tokens = 0;
    obs.output_tokens = 0;
    obs.turn_count = 0;
    MutationEffect::Mutated
}

pub(super) fn model_changed(state: &mut SessionViewState, model: &str) -> MutationEffect {
    state.agent.observable.model = model.to_string();
    MutationEffect::Mutated
}

pub(super) fn mode_changed(state: &mut SessionViewState, mode: &str) -> MutationEffect {
    state.agent.observable.mode = mode.to_string();
    MutationEffect::Mutated
}

pub(super) fn permission_mode_changed(state: &mut SessionViewState, mode: &str) -> MutationEffect {
    state.agent.observable.permission_mode = mode.to_string();
    MutationEffect::Mutated
}

pub(super) fn decision_mode_changed(state: &mut SessionViewState, mode: &str) -> MutationEffect {
    state.agent.observable.decision_mode = mode.to_string();
    MutationEffect::Mutated
}

pub(super) fn sandbox_policy_changed(state: &mut SessionViewState, policy: &str) -> MutationEffect {
    state.agent.observable.sandbox_policy = policy.to_string();
    MutationEffect::Mutated
}

pub(super) fn thinking_changed(state: &mut SessionViewState, raw_json: &str) -> MutationEffect {
    state.agent.observable.thinking_config = normalize_thinking_label(raw_json);
    MutationEffect::Mutated
}

pub(super) fn continuation_gate_changed(
    state: &mut SessionViewState,
    summary: &ContinuationGateSummary,
) -> MutationEffect {
    let current = state.agent.observable.status;
    let next = if !summary.open && summary.closed_reason == Some(GateCloseReason::UserSuspend) {
        AgentStatus::Suspended
    } else if summary.open && current == AgentStatus::Suspended {
        AgentStatus::Running
    } else {
        return MutationEffect::NoOp;
    };
    if current == next {
        return MutationEffect::NoOp;
    }
    state.agent.observable.status = next;
    MutationEffect::Mutated
}

/// Map the raw `ThinkingConfig` JSON to its short label form for status-bar
/// and picker display. Effort variants expand to their `level` value
/// (`"low"` / `"medium"` / `"high"`) so the model picker can highlight
/// the precise selection; otherwise the `type` discriminator is the label.
/// Unknown / malformed payloads fall back to "auto" so the UI never renders
/// an empty cell.
fn normalize_thinking_label(raw_json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_json) else {
        return "auto".to_string();
    };
    let ty = value.get("type").and_then(|t| t.as_str()).unwrap_or("auto");
    if ty == "effort"
        && let Some(level) = value.get("level").and_then(|l| l.as_str())
    {
        return level.to_string();
    }
    ty.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_picks_type_field() {
        assert_eq!(
            normalize_thinking_label(r#"{"type":"disabled"}"#),
            "disabled"
        );
        assert_eq!(normalize_thinking_label(r#"{"type":"auto"}"#), "auto");
    }

    #[test]
    fn normalize_effort_returns_level() {
        assert_eq!(
            normalize_thinking_label(r#"{"type":"effort","level":"low"}"#),
            "low"
        );
        assert_eq!(
            normalize_thinking_label(r#"{"type":"effort","level":"medium"}"#),
            "medium"
        );
        assert_eq!(
            normalize_thinking_label(r#"{"type":"effort","level":"high"}"#),
            "high"
        );
    }

    #[test]
    fn normalize_effort_without_level_falls_back_to_type() {
        assert_eq!(normalize_thinking_label(r#"{"type":"effort"}"#), "effort");
    }

    #[test]
    fn normalize_falls_back_to_auto_on_garbage() {
        assert_eq!(normalize_thinking_label("not-json"), "auto");
        assert_eq!(normalize_thinking_label(r#"{"missing":"type"}"#), "auto");
    }
}
