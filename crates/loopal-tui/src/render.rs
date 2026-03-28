use ratatui::prelude::*;

use crate::app::{App, SubPage};
use crate::views;
use crate::views::input_view;

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Computed frame layout for one render pass.
///
/// Pure function of (terminal size, agent count, input height) → rectangles.
/// Separates "where things go" from "what renders there."
struct FrameLayout {
    content: Rect,      // f₁: workflow output (elastic)
    agents: Rect,       // f₂: agent status panel (dynamic 0-N)
    separator: Rect,    // f₃: dim dashed line (1)
    retry_banner: Rect, // f₃b: transient retry error banner (0-1)
    input: Rect,        // f₄: command prompt (1..8 rows, dynamic)
    status: Rect,       // f₅: unified status bar (1)
    /// Merged area for sub-page pickers (replaces f₁..f₄).
    picker: Rect,
}

impl FrameLayout {
    fn compute(size: Rect, agent_panel_h: u16, banner_h: u16, input_h: u16) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(agent_panel_h),
                Constraint::Length(1),
                Constraint::Length(banner_h),
                Constraint::Length(input_h),
                Constraint::Length(1),
            ])
            .split(size);

        let [content, agents, separator, retry_banner, input, status] = [
            chunks[0], chunks[1], chunks[2], chunks[3], chunks[4], chunks[5],
        ];

        let picker = Rect::new(
            content.x,
            content.y,
            content.width,
            content.height + agents.height + separator.height + retry_banner.height + input.height,
        );

        Self {
            content,
            agents,
            separator,
            retry_banner,
            input,
            status,
            picker,
        }
    }
}

// ---------------------------------------------------------------------------
// Composition: ui = Σ f_i(state_i)
// ---------------------------------------------------------------------------

/// Compose all views into the frame.
///
/// ```text
/// ui = Σ f_i(state_i)
///
/// f_i   view        state slice                               height
/// ────  ──────────  ──────────────────────────────────────────  ─────────
/// f₁    content     messages, streaming, thinking, scroll      Min(3)
/// f₂    agents      agents, focused_agent                      dynamic
/// f₃    separator   (none)                                     1
/// f₃b   banner      retry_banner                               0-1
/// f₄    input       input_text, cursor, inbox_count            1..8 dynamic
/// f₅    status      mode, model, tokens, elapsed, thinking     1
/// ```
pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let state = app.session.lock();

    // Compute dynamic input height based on content
    let inbox_count = state.inbox.len();
    let pw = input_view::prefix_width(inbox_count, app.pending_image_count());
    let input_h = input_view::input_height(&app.input, size.width, pw);
    let banner_h = views::retry_banner::banner_height(&state.retry_banner);

    let layout = FrameLayout::compute(
        size,
        views::agent_panel::panel_height(&state.agents),
        banner_h,
        input_h,
    );

    // Sub-page mode: picker replaces f₁..f₄, only f₅ remains
    if let Some(ref sub_page) = app.sub_page {
        match sub_page {
            SubPage::ModelPicker(p) => views::picker::render_picker(f, p, layout.picker),
            SubPage::RewindPicker(r) => {
                views::rewind_picker::render_rewind_picker(f, r, layout.picker);
            }
        }
        views::unified_status::render_unified_status(f, &state, layout.status);
        return;
    }

    // --- Σ f_i(state_i) ---
    app.content_overflows = views::progress::render_progress(
        f,
        &state,
        app.scroll_offset,
        &mut app.line_cache,
        layout.content,
    );
    views::agent_panel::render_agent_panel(
        f,
        &state.agents,
        state.focused_agent.as_deref(),
        layout.agents,
    );
    views::separator::render_separator(f, layout.separator);
    if let Some(ref msg) = state.retry_banner {
        views::retry_banner::render_retry_banner(f, msg, layout.retry_banner);
    }
    views::unified_status::render_unified_status(f, &state, layout.status);

    // Extract overlay data, release domain state lock
    let pending_perm = state.pending_permission.clone();
    let pending_question = state.pending_question.clone();
    let topology_data = if app.show_topology {
        use loopal_protocol::AgentStatus;
        let root_status = if state.agent_idle {
            AgentStatus::WaitingForInput
        } else {
            AgentStatus::Running
        };
        Some(views::topology_overlay::extract_topology(
            &state.agents,
            &state.model,
            root_status,
            state.turn_elapsed(),
        ))
    } else {
        None
    };
    drop(state);

    // f₄ rendered post-lock (borrows app.input, not SessionState)
    let image_count = app.pending_image_count();
    views::input_view::render_input(
        f,
        &app.input,
        app.input_cursor,
        inbox_count,
        image_count,
        app.input_scroll,
        layout.input,
    );

    // Overlay layer: conditional popups on top of base composition
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
