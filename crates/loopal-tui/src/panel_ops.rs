//! Panel zone focus navigation — enter, tab, cycle, ensure focus.

use loopal_protocol::AgentStatus;

use crate::app::{App, FocusMode, PanelKind};
use crate::views::agent_panel::MAX_VISIBLE;
use crate::views::bg_tasks_panel;

/// Enter Panel focus mode. Picks the first panel with content.
pub fn enter_panel(app: &mut App) {
    let has_agents = has_live_agents(app);
    let has_bg = bg_tasks_panel::bg_panel_height(&app.bg_snapshots) > 0;
    if !has_agents && !has_bg {
        return;
    }
    let kind = if has_agents {
        PanelKind::Agents
    } else {
        PanelKind::BgTasks
    };
    app.focus_mode = FocusMode::Panel(kind);
    ensure_focus(app, kind);
}

/// Tab within the panel zone: switch panel if both have content, else cycle.
pub fn panel_tab(app: &mut App) {
    let kind = match app.focus_mode {
        FocusMode::Panel(k) => k,
        _ => return,
    };
    let has_agents = has_live_agents(app);
    let has_bg = bg_tasks_panel::bg_panel_height(&app.bg_snapshots) > 0;

    if has_agents && has_bg {
        let next = match kind {
            PanelKind::Agents => PanelKind::BgTasks,
            PanelKind::BgTasks => PanelKind::Agents,
        };
        app.focus_mode = FocusMode::Panel(next);
        ensure_focus(app, next);
    } else {
        cycle_panel_focus(app, true);
    }
}

/// Navigate up/down within the currently active panel.
pub fn cycle_panel_focus(app: &mut App, forward: bool) {
    match app.focus_mode {
        FocusMode::Panel(PanelKind::Agents) => cycle_agent_focus(app, forward),
        FocusMode::Panel(PanelKind::BgTasks) => cycle_bg_task_focus(app, forward),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Agent panel focus cycling
// ---------------------------------------------------------------------------

fn cycle_agent_focus(app: &mut App, forward: bool) {
    let keys = live_agent_keys(app);
    if keys.is_empty() {
        app.focused_agent = None;
        app.focus_mode = FocusMode::Input;
        app.agent_panel_offset = 0;
        return;
    }
    app.focused_agent = Some(next_in_list(&keys, app.focused_agent.as_deref(), forward));
    if let Some(ref focused) = app.focused_agent
        && let Some(idx) = keys.iter().position(|k| k == focused)
    {
        adjust_agent_scroll(app, idx, keys.len());
    }
}

fn cycle_bg_task_focus(app: &mut App, forward: bool) {
    let ids = bg_tasks_panel::running_task_ids(&app.bg_snapshots);
    if ids.is_empty() {
        app.focused_bg_task = None;
        if has_live_agents(app) {
            app.focus_mode = FocusMode::Panel(PanelKind::Agents);
            ensure_focus(app, PanelKind::Agents);
        } else {
            app.focus_mode = FocusMode::Input;
        }
        return;
    }
    app.focused_bg_task = Some(next_in_list(&ids, app.focused_bg_task.as_deref(), forward));
}

/// Ensure the focused item exists for the given panel kind.
fn ensure_focus(app: &mut App, kind: PanelKind) {
    match kind {
        PanelKind::Agents => {
            let keys = live_agent_keys(app);
            let needs = match &app.focused_agent {
                None => true,
                Some(name) => !keys.contains(name),
            };
            if needs {
                cycle_agent_focus(app, true);
            }
        }
        PanelKind::BgTasks => {
            let ids = bg_tasks_panel::running_task_ids(&app.bg_snapshots);
            let needs = match &app.focused_bg_task {
                None => true,
                Some(id) => !ids.contains(id),
            };
            if needs {
                cycle_bg_task_focus(app, true);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn live_agent_keys(app: &App) -> Vec<String> {
    let state = app.session.lock();
    let active = &state.active_view;
    state
        .agents
        .iter()
        .filter(|(k, a)| k.as_str() != active && is_agent_live(&a.observable.status))
        .map(|(k, _)| k.clone())
        .collect()
}

pub(crate) fn has_live_agents(app: &App) -> bool {
    let state = app.session.lock();
    let active = &state.active_view;
    state
        .agents
        .iter()
        .any(|(k, a)| k.as_str() != active && is_agent_live(&a.observable.status))
}

/// Pick the next (or previous) item in a list, wrapping around.
fn next_in_list(items: &[String], current: Option<&str>, forward: bool) -> String {
    let pos = current.and_then(|c| items.iter().position(|k| k == c));
    match pos {
        Some(i) => {
            if forward {
                items[(i + 1) % items.len()].clone()
            } else {
                items[(i + items.len() - 1) % items.len()].clone()
            }
        }
        None => {
            if forward {
                items[0].clone()
            } else {
                items[items.len() - 1].clone()
            }
        }
    }
}

/// Ensure the focused agent at `focused_idx` is visible within the scroll window.
fn adjust_agent_scroll(app: &mut App, focused_idx: usize, total: usize) {
    if total <= MAX_VISIBLE {
        app.agent_panel_offset = 0;
        return;
    }
    if focused_idx < app.agent_panel_offset {
        app.agent_panel_offset = focused_idx;
    } else if focused_idx >= app.agent_panel_offset + MAX_VISIBLE {
        app.agent_panel_offset = focused_idx + 1 - MAX_VISIBLE;
    }
    app.agent_panel_offset = app
        .agent_panel_offset
        .min(total.saturating_sub(MAX_VISIBLE));
}

fn is_agent_live(status: &AgentStatus) -> bool {
    !matches!(status, AgentStatus::Finished | AgentStatus::Error)
}
