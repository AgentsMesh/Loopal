use ratatui::prelude::*;

use crate::app::{App, SubPage};
use crate::views;

/// Compose all views into the seven-area layout.
///
/// ```text
/// ┌───────────────────────────────────────────────┐
/// │ Progress Area (elastic main region)            │
/// ├───────────────────────────────────────────────┤
/// │ [~] agent (tools, turns, tokens)  (0-1 line)  │
/// ├───────────────────────────────────────────────┤
/// │ [src→tgt] preview...      (message feed, 0-3) │
/// ├───────────────────────────────────────────────┤
/// │ ● Status  3m24s  ↑1.2k ↓0.8k  (Task Summary) │
/// ├───────────────────────────────────────────────┤
/// │  pending message...       (Inbox, dynamic 0-3) │
/// ├───────────────────────────────────────────────┤
/// │ > Input                              (3 lines) │
/// ├───────────────────────────────────────────────┤
/// │ ACT  model  ctx:45k/200k  turns:3   (1 line)  │
/// └───────────────────────────────────────────────┘
/// ```
pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    let state = app.session.lock();

    let inbox_h = views::inbox_view::inbox_height(&state.inbox);
    let panel_h = views::subagent_panel::panel_height(&state.agents);
    let feed_h = views::message_log_view::feed_height(&state.message_feed);

    let constraints = vec![
        Constraint::Min(5),           // progress area
        Constraint::Length(panel_h),  // agent panel (0 or 1)
        Constraint::Length(feed_h),   // message feed (0-3)
        Constraint::Length(1),        // task summary
        Constraint::Length(inbox_h),  // inbox (dynamic 0-3)
        Constraint::Length(3),        // input
        Constraint::Length(1),        // status bar
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    let [progress_area, panel_area, feed_area, summary_area, inbox_area, input_area, status_area] =
        [chunks[0], chunks[1], chunks[2], chunks[3], chunks[4], chunks[5], chunks[6]];

    // Sub-page replaces progress + panel + feed + summary + inbox + input
    if let Some(ref sub_page) = app.sub_page {
        let picker_area = Rect::new(
            progress_area.x,
            progress_area.y,
            progress_area.width,
            progress_area.height
                + panel_area.height
                + feed_area.height
                + summary_area.height
                + inbox_area.height
                + input_area.height,
        );

        match sub_page {
            SubPage::ModelPicker(picker) => {
                views::picker::render_picker(f, picker, picker_area);
            }
        }

        views::status_bar::render_status_bar(f, &state, status_area);
        return;
    }

    // Normal seven-area render
    views::progress::render_progress(
        f,
        &state,
        app.scroll_offset,
        &mut app.line_cache,
        progress_area,
    );
    views::subagent_panel::render_subagent_panel(
        f,
        &state.agents,
        state.focused_agent.as_deref(),
        panel_area,
    );
    views::message_log_view::render_message_feed(f, &state.message_feed, feed_area);
    views::task_summary::render_task_summary(f, &state, summary_area);
    views::inbox_view::render_inbox(f, &state.inbox, inbox_area);
    views::status_bar::render_status_bar(f, &state, status_area);

    // Tool confirm popup overlay — clone permission data, then drop lock
    let pending_perm = state.pending_permission.clone();
    drop(state);

    views::input_view::render_input(f, &app.input, app.input_cursor, input_area);

    if let Some(ref perm) = pending_perm {
        views::tool_confirm::render_tool_confirm(f, &perm.name, &perm.input, size);
    }

    // Autocomplete command menu overlay (above input area)
    if let Some(ref ac) = app.autocomplete {
        views::command_menu::render_command_menu(f, ac, &app.commands, input_area);
    }
}
