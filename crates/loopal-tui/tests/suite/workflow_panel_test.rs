use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse, WorkflowNodeId,
    WorkflowRunId, WorkflowRunState, WorkflowRunSummary, WorkflowRunsSnapshot, WorkflowStateCounts,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, PanelKind};
use loopal_tui::views::workflows_panel::{
    MAX_WORKFLOW_VISIBLE, render_workflows_panel, workflow_ids, workflows_panel_height,
};
use loopal_view_state::{SessionViewState, ViewSnapshot};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;
use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    App::new(
        SessionController::new(
            control_tx,
            permission_tx,
            question_tx,
            Default::default(),
            std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
        ),
        std::env::temp_dir(),
    )
}

fn summary(id: &str, state: WorkflowRunState, revision: u64) -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new(id),
        run_goal: format!("goal for {id}"),
        state,
        revision,
        output_node: WorkflowNodeId::new("done"),
        counts: WorkflowStateCounts {
            pending: 1,
            ready: 0,
            active: u32::from(state == WorkflowRunState::Running),
            succeeded: u32::from(state == WorkflowRunState::Succeeded),
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 10,
        updated_at_unix_ms: 10 + revision,
    }
}

include!("workflow_panel_test/state.rs");
include!("workflow_panel_test/render.rs");
