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
    fn max_visible(&self) -> usize {
        tasks_panel::MAX_TASK_VISIBLE
    }
    fn item_ids(&self, app: &App) -> Vec<String> {
        tasks_panel::task_ids(&app.task_snapshots)
    }
    fn height(&self, app: &App, _state: &SessionState) -> u16 {
        tasks_panel::tasks_panel_height(&app.task_snapshots)
    }
    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        _state: &SessionState,
        focused: Option<&str>,
        elapsed: Duration,
        area: Rect,
    ) {
        let offset = app.section(PanelKind::Tasks).scroll_offset;
        tasks_panel::render_tasks_panel(
            f,
            &app.task_snapshots,
            focused,
            elapsed,
            offset,
            area,
        );
    }
}
