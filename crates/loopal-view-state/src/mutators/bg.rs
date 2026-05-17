use loopal_protocol::{BgTaskSnapshot, BgTaskStatus};

use crate::state::{BgTaskView, SessionViewState};

use super::MutationEffect;

pub(super) fn spawned(
    state: &mut SessionViewState,
    id: &str,
    description: &str,
    created_at_unix_ms: u64,
) -> MutationEffect {
    let view = BgTaskView::from_snapshot(BgTaskSnapshot {
        id: id.to_string(),
        description: description.to_string(),
        status: BgTaskStatus::Running,
        exit_code: None,
        created_at_unix_ms,
    });
    state.bg_tasks.insert(id.to_string(), view);
    MutationEffect::Mutated
}

pub(super) fn output(state: &mut SessionViewState, id: &str, delta: &str) -> MutationEffect {
    match state.bg_tasks.get_mut(id) {
        Some(view) => {
            view.output.push_str(delta);
            MutationEffect::Mutated
        }
        None => MutationEffect::NoOp,
    }
}

pub(super) fn completed(
    state: &mut SessionViewState,
    id: &str,
    status: BgTaskStatus,
    exit_code: Option<i32>,
    output: &str,
) -> MutationEffect {
    if let Some(view) = state.bg_tasks.get_mut(id) {
        view.status = status;
        view.exit_code = exit_code;
        view.output = output.to_string();
        return MutationEffect::Mutated;
    }
    let view = BgTaskView {
        id: id.to_string(),
        description: String::new(),
        status,
        exit_code,
        output: output.to_string(),
        created_at_unix_ms: 0,
    };
    state.bg_tasks.insert(id.to_string(), view);
    MutationEffect::Mutated
}
