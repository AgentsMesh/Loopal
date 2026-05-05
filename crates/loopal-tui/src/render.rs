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
    let active = state.active_view.clone();
    let vc = app.view_client_for(&active);
    let vc_guard = vc.state();
    let conv = vc_guard.conversation();

    let pw = input_view::prefix_width(app.pending_image_count());
    let banner_h = views::retry_banner::banner_height(&conv.retry_banner);
    let breadcrumb_h = u16::from(state.active_view != loopal_session::ROOT_AGENT);
    let elapsed = conv.turn_elapsed();

    // PendingQuestion is cloned here so we can drop `vc_guard` before the
    // input area renders below. Questions are typically 1-4 short strings
    // and TUI redraws are event-driven (not 60fps), so the clone cost is
    // negligible. Permission uses the lighter `prepare` borrow path because
    // its `input` JSON can be much larger.
    let pending_question = conv.pending_question.clone();
    let prepared_perm = conv
        .pending_permission
        .as_ref()
        .map(views::permission_inline::prepare);

    let input_h = if let Some(ref q) = pending_question {
        views::question_inline::height(q, size.width)
    } else if let Some(ref prep) = prepared_perm {
        views::permission_inline::height_of(prep)
    } else {
        input_view::input_height(&app.input, size.width, pw)
    };

    let panel_zone_h = crate::render_panel::panel_zone_height(app, &state);
    let layout = FrameLayout::compute(size, breadcrumb_h, panel_zone_h, banner_h, input_h);

    if let Some(ref mut sub_page) = app.sub_page {
        render_sub_page(f, sub_page, &app.bg_task_details, layout.picker);
        views::unified_status::render_unified_status(f, app, &state, conv, layout.status);
        return;
    }

    if breadcrumb_h > 0 {
        views::breadcrumb::render_breadcrumb(f, &state.active_view, layout.breadcrumb);
    }
    app.content_scroll.render(f, conv, layout.content);
    crate::render_panel::render_panel_zone(f, app, &state, elapsed, layout.agents);
    views::separator::render_separator(f, layout.separator);
    if let Some(ref msg) = conv.retry_banner {
        views::retry_banner::render_retry_banner(f, msg, layout.retry_banner);
    }
    views::unified_status::render_unified_status(f, app, &state, conv, layout.status);

    let topology_data = if app.show_topology {
        Some(extract_topology(app, &state, elapsed))
    } else {
        None
    };
    drop(vc_guard);
    drop(state);

    if let Some(ref question) = pending_question {
        let status = app.current_transient_status().map(String::from);
        views::question_inline::render(f, question, layout.input, status.as_deref());
    } else if let Some(ref prep) = prepared_perm {
        let status = app.current_transient_status().map(String::from);
        views::permission_inline::render_prepared(f, prep, layout.input, status.as_deref());
    } else {
        let image_count = app.pending_image_count();
        views::input_view::render_input(
            f,
            &app.input,
            app.input_cursor,
            image_count,
            app.input_scroll,
            layout.input,
        );
        if let Some(ref ac) = app.autocomplete {
            views::command_menu::render_command_menu(f, ac, layout.input);
        }
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

fn extract_topology(
    app: &App,
    state: &loopal_session::state::SessionState,
    elapsed: std::time::Duration,
) -> Vec<views::topology_overlay::TopologyNode> {
    use indexmap::IndexMap;
    use loopal_protocol::AgentStatus;
    use views::topology_overlay::AgentTopologySnapshot;

    let root_idle = matches!(
        app.observable_for(&state.active_view).status,
        AgentStatus::WaitingForInput | AgentStatus::Finished | AgentStatus::Error
    );
    let root_status = if root_idle {
        AgentStatus::WaitingForInput
    } else {
        AgentStatus::Running
    };

    let agents: IndexMap<String, AgentTopologySnapshot> = app
        .view_clients
        .iter()
        .map(|(name, vc)| {
            let guard = vc.state();
            let view = &guard.state().agent;
            (
                name.clone(),
                AgentTopologySnapshot {
                    status: view.observable.status,
                    model: view.observable.model.clone(),
                    elapsed: view.elapsed(),
                    tools_in_flight: view.observable.tools_in_flight,
                    parent: view.parent.clone(),
                    children: view.children.clone(),
                },
            )
        })
        .collect();

    let root_model = app.observable_for(&state.active_view).model;
    views::topology_overlay::extract_topology(&agents, &root_model, root_status, elapsed)
}
