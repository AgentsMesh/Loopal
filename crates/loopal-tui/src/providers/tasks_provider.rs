//! Structured tasks panel provider.

use std::time::Duration;

use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::tasks_panel;

pub struct TasksPanelProvider;

impl PanelProvider for TasksPanelProvider {
    fn kind(&self) -> PanelKind {
        PanelKind::Tasks
    }
    fn title(&self) -> &'static str {
        "Tasks"
    }
    fn max_visible(&self) -> usize {
        tasks_panel::MAX_TASK_VISIBLE
    }
    fn item_ids(&self, app: &App, state: &SessionState) -> Vec<String> {
        tasks_panel::task_ids(&app.view_client_for(&state.active_view).task_snapshots())
    }
    fn count(&self, app: &App, state: &SessionState) -> usize {
        tasks_panel::active_count(&app.view_client_for(&state.active_view).task_snapshots())
    }
    fn height(&self, app: &App, state: &SessionState) -> u16 {
        tasks_panel::tasks_panel_height(&app.view_client_for(&state.active_view).task_snapshots())
    }
    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        state: &SessionState,
        focused: Option<&str>,
        elapsed: Duration,
        area: Rect,
    ) {
        let offset = app.section(PanelKind::Tasks).scroll_offset;
        let snapshots = app.view_client_for(&state.active_view).task_snapshots();
        tasks_panel::render_tasks_panel(f, &snapshots, focused, elapsed, offset, area);
    }
}
