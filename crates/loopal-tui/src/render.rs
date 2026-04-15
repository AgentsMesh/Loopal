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
    let breadcrumb_h = if state.active_view != loopal_session::ROOT_AGENT {
        1
    } else {
        0
    };

    let agent_panel_h =
        views::agent_panel::panel_height(&state.agents, &state.active_view, app.agent_panel_offset);
    let bg_panel_h = views::bg_tasks_panel::bg_panel_height(&app.bg_snapshots);

    let layout = FrameLayout::compute(
        size,
        breadcrumb_h,
        agent_panel_h + bg_panel_h,
        banner_h,
        input_h,
    );

    // Sub-page mode: picker replaces f₁..f₄, only f₅ remains
    if let Some(ref mut sub_page) = app.sub_page {
        match sub_page {
            SubPage::ModelPicker(p) | SubPage::SessionPicker(p) => {
                views::picker::render_picker(f, p, layout.picker);
            }
            SubPage::RewindPicker(r) => {
                views::rewind_picker::render_rewind_picker(f, r, layout.picker);
            }
            SubPage::StatusPage(s) => {
                views::status_page::render_status_page(f, s, layout.picker);
            }
            SubPage::McpPage(s) => {
                views::mcp_page::render_mcp_page(f, s, layout.picker);
            }
            SubPage::SkillsPage(s) => {
                views::skills_page::render_skills_page(f, s, layout.picker);
            }
            SubPage::BgTaskLog(s) => {
                views::bg_task_log::render_bg_task_log(f, s, &app.bg_task_details, layout.picker);
            }
        }
        views::unified_status::render_unified_status(f, &state, layout.status);
        return;
    }

    // --- Σ f_i(state_i) ---
    if breadcrumb_h > 0 {
        views::breadcrumb::render_breadcrumb(f, &state.active_view, layout.breadcrumb);
    }
    app.content_scroll.render(f, &state, layout.content);
    let viewing = if state.active_view != loopal_session::ROOT_AGENT {
        Some(state.active_view.as_str())
    } else {
        None
    };
    // Agent panel + background tasks panel share the `agents` rect.
    // Guard: skip panel rendering if the available area is zero height.
    let focused_bg = app.focused_bg_task.as_deref();
    if layout.agents.height > 0 && bg_panel_h > 0 && agent_panel_h > 0 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(agent_panel_h),
                Constraint::Length(bg_panel_h),
            ])
            .split(layout.agents);
        views::agent_panel::render_agent_panel(
            f,
            &state.agents,
            app.focused_agent.as_deref(),
            viewing,
            &state.active_view,
            app.agent_panel_offset,
            split[0],
        );
        views::bg_tasks_panel::render_bg_tasks(
            f,
            &app.bg_snapshots,
            focused_bg,
            conv.turn_elapsed(),
            split[1],
        );
    } else if layout.agents.height > 0 && bg_panel_h > 0 {
        views::bg_tasks_panel::render_bg_tasks(
            f,
            &app.bg_snapshots,
            focused_bg,
            conv.turn_elapsed(),
            layout.agents,
        );
    } else if layout.agents.height > 0 {
        views::agent_panel::render_agent_panel(
            f,
            &state.agents,
            app.focused_agent.as_deref(),
            viewing,
            &state.active_view,
            app.agent_panel_offset,
            layout.agents,
        );
    }
    views::separator::render_separator(f, layout.separator);
    if let Some(ref msg) = conv.retry_banner {
        views::retry_banner::render_retry_banner(f, msg, layout.retry_banner);
    }
    views::unified_status::render_unified_status(f, &state, layout.status);

    // Extract overlay data, release domain state lock
    let pending_perm = conv.pending_permission.clone();
    let pending_question = conv.pending_question.clone();
    let topology_data = if app.show_topology {
        use loopal_protocol::AgentStatus;
        let root_status = if state.is_active_agent_idle() {
            AgentStatus::WaitingForInput
        } else {
            AgentStatus::Running
        };
        Some(views::topology_overlay::extract_topology(
            &state.agents,
            &state.model,
            root_status,
            conv.turn_elapsed(),
        ))
    } else {
        None
    };
    drop(state);

    let image_count = app.pending_image_count();
    views::input_view::render_input(
        f,
        &app.input,
        app.input_cursor,
        image_count,
        app.input_scroll,
        layout.input,
    );

    // Overlay layer
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
