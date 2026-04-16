//! Frame composition — combines all views into the terminal frame.

use ratatui::prelude::*;

use crate::app::{App, SubPage};
use crate::render_layout::FrameLayout;
use crate::views;
use crate::views::input_view;

/// Compose all views into the frame.
pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let state = app.session.lock();

    let pw = input_view::prefix_width(app.pending_image_count());
    let input_h = input_view::input_height(&app.input, size.width, pw);
    let conv = state.active_conversation();
    let banner_h = views::retry_banner::banner_height(&conv.retry_banner);
    let breadcrumb_h = u16::from(state.active_view != loopal_session::ROOT_AGENT);
    let elapsed = conv.turn_elapsed();

    let panel_zone_h: u16 = app
        .panel_registry
        .providers()
        .iter()
        .map(|p| p.height(app, &state))
        .sum();
    let layout = FrameLayout::compute(size, breadcrumb_h, panel_zone_h, banner_h, input_h);

    if let Some(ref mut sub_page) = app.sub_page {
        render_sub_page(f, sub_page, &app.bg_task_details, layout.picker);
        views::unified_status::render_unified_status(f, &state, layout.status);
        return;
    }

    if breadcrumb_h > 0 {
        views::breadcrumb::render_breadcrumb(f, &state.active_view, layout.breadcrumb);
    }
    app.content_scroll.render(f, &state, layout.content);
    render_panel_zone(f, app, &state, elapsed, layout.agents);
    views::separator::render_separator(f, layout.separator);
    if let Some(ref msg) = conv.retry_banner {
        views::retry_banner::render_retry_banner(f, msg, layout.retry_banner);
    }
    views::unified_status::render_unified_status(f, &state, layout.status);

    let pending_perm = conv.pending_permission.clone();
    let pending_question = conv.pending_question.clone();
    let topology_data = if app.show_topology {
        Some(extract_topology(&state, elapsed))
    } else {
        None
    };
    drop(state);

    let image_count = app.pending_image_count();
    views::input_view::render_input(
        f, &app.input, app.input_cursor, image_count, app.input_scroll, layout.input,
    );
    if let Some(ref perm) = pending_perm {
        views::tool_confirm::render_tool_confirm(f, &perm.name, &perm.input, size);
    }
    if let Some(ref question) = pending_question {
        views::question_dialog::render_question_dialog(f, question, size);
    }
    if let Some(ref ac) = app.autocomplete {
        views::command_menu::render_command_menu(f, ac, layout.input);
    }
    if let Some(ref nodes) = topology_data {
        views::topology_overlay::render_topology_overlay(f, nodes, size);
    }
}

fn render_sub_page(
    f: &mut Frame,
    sub_page: &mut SubPage,
    bg_details: &[loopal_protocol::BgTaskDetail],
    area: Rect,
) {
    match sub_page {
        SubPage::ModelPicker(p) | SubPage::SessionPicker(p) => {
            views::picker::render_picker(f, p, area);
        }
        SubPage::RewindPicker(r) => views::rewind_picker::render_rewind_picker(f, r, area),
        SubPage::StatusPage(s) => views::status_page::render_status_page(f, s, area),
        SubPage::McpPage(s) => views::mcp_page::render_mcp_page(f, s, area),
        SubPage::SkillsPage(s) => views::skills_page::render_skills_page(f, s, area),
        SubPage::BgTaskLog(s) => views::bg_task_log::render_bg_task_log(f, s, bg_details, area),
    }
}

/// Render the panel zone using registered providers.
fn render_panel_zone(
    f: &mut Frame,
    app: &App,
    state: &loopal_session::state::SessionState,
    elapsed: std::time::Duration,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }
    let heights: Vec<_> = app
        .panel_registry
        .providers()
        .iter()
        .map(|p| (p.as_ref(), p.height(app, state)))
        .filter(|(_, h)| *h > 0)
        .collect();
    if heights.is_empty() {
        return;
    }
    let constraints: Vec<Constraint> = heights.iter().map(|(_, h)| Constraint::Length(*h)).collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for ((provider, _), &chunk) in heights.iter().zip(chunks.iter()) {
        let focused = app.section(provider.kind()).focused.as_deref();
        provider.render(f, app, state, focused, elapsed, chunk);
    }
}

fn extract_topology(
    state: &loopal_session::state::SessionState,
    elapsed: std::time::Duration,
) -> Vec<views::topology_overlay::TopologyNode> {
    use loopal_protocol::AgentStatus;
    let root_status = if state.is_active_agent_idle() {
        AgentStatus::WaitingForInput
    } else {
        AgentStatus::Running
    };
    views::topology_overlay::extract_topology(&state.agents, &state.model, root_status, elapsed)
}
