//! Agent panel provider — queries session state for live agents.

use std::time::Duration;

use loopal_protocol::AgentStatus;
use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::agent_panel;

pub struct AgentPanelProvider;

impl PanelProvider for AgentPanelProvider {
    fn kind(&self) -> PanelKind {
        PanelKind::Agents
    }
    fn max_visible(&self) -> usize {
        agent_panel::MAX_VISIBLE
    }
    fn item_ids(&self, app: &App) -> Vec<String> {
        let state = app.session.lock();
        live_agent_ids(&state)
    }
    fn height(&self, app: &App, state: &SessionState) -> u16 {
        let offset = app.section(PanelKind::Agents).scroll_offset;
        agent_panel::panel_height(&state.agents, &state.active_view, offset)
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
        let offset = app.section(PanelKind::Agents).scroll_offset;
        agent_panel::render_agent_panel(
            f,
            &state.agents,
            focused,
            &state.active_view,
            offset,
            area,
        );
    }
}

pub(crate) fn live_agent_ids(state: &SessionState) -> Vec<String> {
    state
        .agents
        .iter()
        .filter(|(k, a)| k.as_str() != state.active_view && is_live(&a.observable.status))
        .map(|(k, _)| k.clone())
        .collect()
}

fn is_live(status: &AgentStatus) -> bool {
    !matches!(status, AgentStatus::Finished | AgentStatus::Error)
}
