//! Root workflow runs panel provider.

use std::time::Duration;

use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::workflows_panel;

pub struct WorkflowsPanelProvider;

impl PanelProvider for WorkflowsPanelProvider {
    fn kind(&self) -> PanelKind {
        PanelKind::Workflows
    }

    fn title(&self) -> &'static str {
        "Workflows"
    }

    fn max_visible(&self) -> usize {
        workflows_panel::MAX_WORKFLOW_VISIBLE
    }

    fn item_ids(&self, app: &App, state: &SessionState) -> Vec<String> {
        workflows_panel::workflow_ids(&app.view_client_for(&state.active_view).workflow_snapshots())
    }

    fn count(&self, app: &App, state: &SessionState) -> usize {
        let workflows = app.view_client_for(&state.active_view).workflow_snapshots();
        workflows.active.len() + workflows.recent.len()
    }

    fn height(&self, app: &App, state: &SessionState) -> u16 {
        workflows_panel::workflows_panel_height(
            &app.view_client_for(&state.active_view).workflow_snapshots(),
        )
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        state: &SessionState,
        focused: Option<&str>,
        _animation_elapsed: Duration,
        area: Rect,
    ) {
        let offset = app.section(PanelKind::Workflows).scroll_offset;
        let workflows = app.view_client_for(&state.active_view).workflow_snapshots();
        workflows_panel::render_workflows_panel(f, &workflows, focused, offset, area);
    }
}
