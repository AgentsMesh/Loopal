//! Agent panel provider — queries session state for live agents.

use std::time::Duration;

use loopal_protocol::AgentStatus;
use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::agent_panel::{self, AgentDisplayInfo};

pub struct AgentPanelProvider;

impl PanelProvider for AgentPanelProvider {
    fn kind(&self) -> PanelKind {
        PanelKind::Agents
    }
    fn title(&self) -> &'static str {
        "Agents"
    }
    fn max_visible(&self) -> usize {
        agent_panel::MAX_VISIBLE
    }
    fn item_ids(&self, app: &App, state: &SessionState) -> Vec<String> {
        live_agent_ids(app, state)
    }
    fn count(&self, app: &App, state: &SessionState) -> usize {
        snapshot(app)
            .into_iter()
            .filter(|(name, info)| name != &state.active_view && is_live(&info.status))
            .count()
    }
    fn height(&self, app: &App, state: &SessionState) -> u16 {
        let offset = app.section(PanelKind::Agents).scroll_offset;
        agent_panel::panel_height(&snapshot(app), &state.active_view, offset)
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
            &snapshot(app),
            focused,
            &state.active_view,
            offset,
            area,
        );
    }
}

pub(crate) fn live_agent_ids(app: &App, state: &SessionState) -> Vec<String> {
    snapshot(app)
        .into_iter()
        .filter(|(name, info)| name != &state.active_view && is_live(&info.status))
        .map(|(name, _)| name)
        .collect()
}

/// Read each agent's display info from its `ViewClient` once per query.
pub(crate) fn snapshot(app: &App) -> Vec<(String, AgentDisplayInfo)> {
    app.view_clients
        .iter()
        .map(|(name, vc)| {
            let guard = vc.state();
            let view = &guard.state().agent;
            (
                name.clone(),
                AgentDisplayInfo {
                    status: view.observable.status,
                    last_tool: view.observable.last_tool.clone(),
                    tools_in_flight: view.observable.tools_in_flight,
                    elapsed: view.elapsed(),
                },
            )
        })
        .collect()
}

fn is_live(status: &AgentStatus) -> bool {
    !matches!(status, AgentStatus::Finished | AgentStatus::Error)
}
