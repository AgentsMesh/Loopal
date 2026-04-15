//! Background task state updates from individual lifecycle events.

use loopal_protocol::{AgentEventPayload, BgTaskDetail, BgTaskStatus};

use crate::state::SessionState;

pub(crate) fn apply(state: &mut SessionState, payload: AgentEventPayload) {
    match payload {
        AgentEventPayload::BgTaskSpawned { id, description } => {
            state
                .bg_tasks
                .entry(id.clone())
                .or_insert_with(|| BgTaskDetail {
                    id,
                    description,
                    status: BgTaskStatus::Running,
                    exit_code: None,
                    output: String::new(),
                });
        }
        AgentEventPayload::BgTaskOutput { id, output_delta } => {
            if let Some(task) = state.bg_tasks.get_mut(&id)
                && task.status == BgTaskStatus::Running
            {
                task.output.push_str(&output_delta);
            }
        }
        AgentEventPayload::BgTaskCompleted {
            id,
            status,
            exit_code,
            output,
        } => {
            if let Some(task) = state.bg_tasks.get_mut(&id) {
                task.status = status;
                task.exit_code = exit_code;
                task.output = output;
            } else {
                state.bg_tasks.insert(
                    id.clone(),
                    BgTaskDetail {
                        id,
                        description: String::new(),
                        status,
                        exit_code,
                        output,
                    },
                );
            }
        }
        _ => {}
    }
}
