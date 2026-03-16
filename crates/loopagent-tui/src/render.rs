use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppState, SubPage};
use crate::views;

/// Input area widget rendering
fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Input ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let input_text = Paragraph::new(app.input.as_str());
    f.render_widget(input_text, inner);

    // Place cursor
    f.set_cursor_position((
        inner.x + app.input_cursor as u16,
        inner.y,
    ));
}

/// Compose all views into the full-screen layout.
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Layout: optional plan indicator, chat area, input, status bar
    let is_plan = app.mode == "plan";

    let constraints = if is_plan {
        vec![
            Constraint::Length(3),  // plan indicator
            Constraint::Min(5),    // chat
            Constraint::Length(3), // input
            Constraint::Length(1), // status bar
        ]
    } else {
        vec![
            Constraint::Min(5),    // chat
            Constraint::Length(3), // input
            Constraint::Length(1), // status bar
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    let (chat_idx, input_idx, status_idx) = if is_plan {
        views::plan::render_plan_indicator(f, chunks[0]);
        (1, 2, 3)
    } else {
        (0, 1, 2)
    };

    // Sub-page replaces the chat + input area
    if let Some(ref sub_page) = app.sub_page {
        // Merge chat + input into a single area for the picker
        let picker_area = Rect::new(
            chunks[chat_idx].x,
            chunks[chat_idx].y,
            chunks[chat_idx].width,
            chunks[chat_idx].height + chunks[input_idx].height,
        );

        match sub_page {
            SubPage::ModelPicker(picker) => {
                views::picker::render_picker(f, picker, picker_area);
            }
        }

        views::status_bar::render_status_bar(f, app, chunks[status_idx]);
        return;
    }

    views::chat::render_chat(f, app, chunks[chat_idx]);
    render_input(f, app, chunks[input_idx]);
    views::status_bar::render_status_bar(f, app, chunks[status_idx]);

    // Tool confirm popup overlay
    if let AppState::ToolConfirm { ref name, ref input, .. } = app.state {
        views::tool_confirm::render_tool_confirm(f, name, input, size);
    }

    // Autocomplete command menu overlay (above input area)
    if let Some(ref ac) = app.autocomplete {
        views::command_menu::render_command_menu(f, ac, &app.commands, chunks[input_idx]);
    }
}
