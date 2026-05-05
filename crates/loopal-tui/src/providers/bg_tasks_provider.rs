//! Background shell tasks panel provider.

use std::time::Duration;

use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::bg_tasks_panel;

pub struct BgTasksPanelProvider;

impl PanelProvider for BgTasksPanelProvider {
    fn kind(&self) -> PanelKind {
        PanelKind::BgTasks
    }
    fn title(&self) -> &'static str {
        "Background"
    }
    fn max_visible(&self) -> usize {
        bg_tasks_panel::MAX_BG_VISIBLE
    }
    fn item_ids(&self, app: &App, state: &SessionState) -> Vec<String> {
        bg_tasks_panel::task_ids(&app.view_client_for(&state.active_view).bg_snapshots())
    }
    fn count(&self, app: &App, state: &SessionState) -> usize {
        bg_tasks_panel::running_count(&app.view_client_for(&state.active_view).bg_snapshots())
    }
    fn height(&self, app: &App, state: &SessionState) -> u16 {
        bg_tasks_panel::bg_panel_height(&app.view_client_for(&state.active_view).bg_snapshots())
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
        let snapshots = app.view_client_for(&state.active_view).bg_snapshots();
        bg_tasks_panel::render_bg_tasks(f, &snapshots, focused, elapsed, area);
    }
}
