use loopal_protocol::{MAX_RECENT_WORKFLOW_SUMMARIES, WorkflowRunSummary};

use crate::reducer::WorkflowRevisionGap;
use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn workflow_changed(
    state: &mut SessionViewState,
    summary: &WorkflowRunSummary,
) -> MutationEffect {
    let existing = state
        .workflows
        .active
        .iter()
        .chain(state.workflows.recent.iter())
        .find(|run| run.id == summary.id);
    if let Some(current) = existing {
        if current.revision >= summary.revision || current.state.is_terminal() {
            return MutationEffect::NoOp;
        }
        let expected = current.revision.saturating_add(1);
        if summary.revision != expected {
            return MutationEffect::WorkflowRevisionGap(WorkflowRevisionGap {
                run_id: summary.id.clone(),
                expected_revision: expected,
                actual_revision: summary.revision,
            });
        }
    }

    state.workflows.active.retain(|run| run.id != summary.id);
    state.workflows.recent.retain(|run| run.id != summary.id);
    if summary.state.is_terminal() {
        state.workflows.recent.push(summary.clone());
        state.workflows.recent.sort_by(|left, right| {
            right
                .updated_at_unix_ms
                .cmp(&left.updated_at_unix_ms)
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        state
            .workflows
            .recent
            .truncate(MAX_RECENT_WORKFLOW_SUMMARIES);
    } else {
        state.workflows.active.push(summary.clone());
        state.workflows.active.sort_by(|left, right| {
            left.created_at_unix_ms
                .cmp(&right.created_at_unix_ms)
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
    }
    MutationEffect::Mutated
}
