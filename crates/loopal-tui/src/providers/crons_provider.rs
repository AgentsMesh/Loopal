//! Scheduled cron jobs panel provider.

use std::time::Duration;

use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::crons_panel;

pub struct CronsPanelProvider;

impl PanelProvider for CronsPanelProvider {
    fn kind(&self) -> PanelKind {
        PanelKind::Crons
    }
    fn title(&self) -> &'static str {
        "Scheduled"
    }
    fn max_visible(&self) -> usize {
        crons_panel::MAX_CRON_VISIBLE
    }
    fn item_ids(&self, app: &App, state: &SessionState) -> Vec<String> {
        crons_panel::cron_ids(&app.view_client_for(&state.active_view).cron_snapshots())
    }
    fn count(&self, app: &App, state: &SessionState) -> usize {
        app.view_client_for(&state.active_view)
            .cron_snapshots()
            .len()
    }
    fn height(&self, app: &App, state: &SessionState) -> u16 {
        crons_panel::crons_panel_height(&app.view_client_for(&state.active_view).cron_snapshots())
    }
    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        state: &SessionState,
        focused: Option<&str>,
        _elapsed: Duration,
        area: Rect,
    ) {
        // `_elapsed` is intentionally unused: cron rows have no spinner
        // animation. The countdown ("next 2m 30s") is recomputed inside
        // `render_crons_panel` from `Utc::now()` each frame, so the panel
        // refreshes through the TUI's existing 100ms redraw tick.
        let offset = app.section(PanelKind::Crons).scroll_offset;
        let snapshots = app.view_client_for(&state.active_view).cron_snapshots();
        crons_panel::render_crons_panel(f, &snapshots, focused, offset, area);
    }
}
