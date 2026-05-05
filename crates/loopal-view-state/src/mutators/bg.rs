use loopal_protocol::{BgTaskSnapshot, BgTaskStatus};

use crate::state::{BgTaskView, SessionViewState};

pub(super) fn spawned(state: &mut SessionViewState, id: &str, description: &str) -> bool {
    let view = BgTaskView::from_snapshot(BgTaskSnapshot {
        id: id.to_string(),
        description: description.to_string(),
        status: BgTaskStatus::Running,
        exit_code: None,
    });
    state.bg_tasks.insert(id.to_string(), view);
    true
}

pub(super) fn output(state: &mut SessionViewState, id: &str, delta: &str) -> bool {
    match state.bg_tasks.get_mut(id) {
        Some(view) => {
            view.output.push_str(delta);
            true
        }
        None => false,
    }
}

pub(super) fn completed(
    state: &mut SessionViewState,
    id: &str,
    status: BgTaskStatus,
    exit_code: Option<i32>,
    output: &str,
) -> bool {
    match state.bg_tasks.get_mut(id) {
        Some(view) => {
            view.status = status;
            view.exit_code = exit_code;
            view.output = output.to_string();
            true
        }
        None => false,
    }
}
