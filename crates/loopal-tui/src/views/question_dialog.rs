use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use loopal_session::PendingQuestion;

/// Render the AskUser question dialog popup.
pub fn render_question_dialog(f: &mut Frame, q: &PendingQuestion, area: Rect) {
    let popup_width = (area.width * 70 / 100).clamp(40, 90);
    let popup_height = (area.height * 60 / 100).clamp(10, 30);
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    // Defensive: bail if indices are out of bounds
    let Some(question) = q.questions.get(q.current_question) else {
        return;
    };
    let Some(selected) = q.selected.get(q.current_question) else {
        return;
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Question {}/{} ",
            q.current_question + 1,
            q.questions.len()
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let max_lines = inner.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    // Question text
    lines.push(Line::from(Span::styled(
        &question.question,
        Style::default().bold(),
    )));
    lines.push(Line::from(""));

    // Options
    for (i, opt) in question.options.iter().enumerate() {
        if lines.len() >= max_lines.saturating_sub(2) {
            break;
        }
        let is_cursor = i == q.cursor;
        let is_selected = selected.get(i).copied().unwrap_or(false);

        let checkbox = if question.allow_multiple {
            if is_selected { "[x] " } else { "[ ] " }
        } else if is_selected {
            "(o) "
        } else {
            "( ) "
        };

        let style = if is_cursor {
            Style::default().fg(Color::Yellow).bold()
        } else if is_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };

        let prefix = if is_cursor { "▸ " } else { "  " };
        let label = format!("{prefix}{checkbox}{}", opt.label);
        lines.push(Line::from(Span::styled(label, style)));

        // Description on next line (dimmed)
        if !opt.description.is_empty() && lines.len() < max_lines.saturating_sub(2) {
            let desc = format!("    {}", opt.description);
            lines.push(Line::from(Span::styled(
                desc,
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Footer hint
    lines.push(Line::from(""));
    let hint = if question.allow_multiple {
        "[↑/↓] Navigate  [Space] Toggle  [Enter] Submit  [Esc] Cancel"
    } else {
        "[↑/↓] Navigate  [Enter] Select  [Esc] Cancel"
    };
    lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(Color::Cyan).italic(),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}
