use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_view_state::PendingQuestion;

use super::question_layout::compose;
use super::text_width::display_width;

const MAX_HEIGHT: u16 = 12;
const MIN_HEIGHT: u16 = 3;
const OTHER_LABEL: &str = "Other（自定义输入）";
const FREE_TEXT_PREFIX: &str = "    > ";

pub fn height(q: &PendingQuestion, width: u16) -> u16 {
    let Some(cur) = q.questions.get(q.current_question) else {
        return MIN_HEIGHT;
    };
    let title = title_line(q, cur);
    let question_lines = wrapped_lines(&title, width).len() as u16;
    let options_lines = cur.options.len() as u16;
    let other_line: u16 = 1;
    let free_text_line: u16 = if q.cursor_on_other() { 1 } else { 0 };
    let hint_line: u16 = 1;

    (question_lines + options_lines + other_line + free_text_line + hint_line)
        .clamp(MIN_HEIGHT, MAX_HEIGHT)
}

pub fn render(f: &mut Frame, q: &PendingQuestion, area: Rect, status: Option<&str>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if area.height < MIN_HEIGHT {
        let msg = status.unwrap_or("Screen too small for AskUser dialog");
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                msg,
                Style::default().fg(Color::Yellow).bold(),
            ))),
            area,
        );
        return;
    }
    let Some(cur) = q.questions.get(q.current_question) else {
        return;
    };

    let title_lines = wrapped_lines(&title_line(q, cur), area.width);
    let title_height = title_lines.len();
    let title_styled: Vec<Line> = title_lines
        .into_iter()
        .map(|s| Line::from(Span::styled(s, Style::default().fg(Color::Cyan).bold())))
        .collect();

    let other_cursor = q.cursor_on_other();
    let other_selected = if cur.allow_multiple {
        q.other_is_selected()
    } else {
        other_cursor
    };
    let other_line_widget = option_line(
        OTHER_LABEL,
        other_cursor,
        other_selected,
        cur.allow_multiple,
    );

    let mut option_widgets: Vec<Line> = Vec::with_capacity(cur.options.len());
    for (i, opt) in cur.options.iter().enumerate() {
        let is_cursor = q.cursor() == i;
        let is_selected = is_option_selected(q, cur.allow_multiple, i);
        option_widgets.push(option_line(
            &opt.label,
            is_cursor,
            is_selected,
            cur.allow_multiple,
        ));
    }

    let free_text_widget = if other_cursor {
        Some(Line::from(vec![
            Span::styled(FREE_TEXT_PREFIX, Style::default().fg(Color::DarkGray)),
            Span::raw(q.free_text().to_string()),
        ]))
    } else {
        None
    };

    let hint_text = if let Some(s) = status {
        format!("⚠ {s}")
    } else if cur.allow_multiple {
        "↑↓ Nav · Space Toggle · ⏎ Submit · Esc Cancel".to_string()
    } else {
        "↑↓ Nav · ⏎ Submit · Esc Cancel".to_string()
    };
    let hint_style = if status.is_some() {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::DarkGray).italic()
    };
    let hint_widget = Line::from(Span::styled(hint_text, hint_style));

    let (lines, free_text_row) = compose(
        area.height as usize,
        title_height,
        title_styled,
        option_widgets,
        q.cursor(),
        other_line_widget,
        free_text_widget,
        hint_widget,
    );

    f.render_widget(Paragraph::new(lines), area);

    if let Some(row) = free_text_row {
        let prefix_w = display_width(FREE_TEXT_PREFIX) as u16;
        let typed = char_prefix(q.free_text(), q.free_text_cursor());
        let typed_w = display_width(&typed) as u16;
        let cursor_col = area.x + prefix_w + typed_w;
        let cursor_row = area.y + row as u16;
        f.set_cursor_position((cursor_col, cursor_row));
    }
}

fn title_line(q: &PendingQuestion, cur: &loopal_protocol::Question) -> String {
    if q.questions.len() > 1 {
        format!(
            "? {} ({}/{})",
            cur.question,
            q.current_question + 1,
            q.questions.len()
        )
    } else {
        format!("? {}", cur.question)
    }
}

fn wrapped_lines(text: &str, width: u16) -> Vec<String> {
    let w = (width as usize).max(1);
    textwrap::wrap(text, w)
        .into_iter()
        .map(|c| c.to_string())
        .collect()
}

fn char_prefix(s: &str, char_count: usize) -> String {
    s.chars().take(char_count).collect()
}

fn is_option_selected(q: &PendingQuestion, multi: bool, idx: usize) -> bool {
    if multi {
        q.selection().get(idx).copied().unwrap_or(false)
    } else {
        q.cursor() == idx
    }
}

fn option_line(label: &str, is_cursor: bool, is_selected: bool, multi: bool) -> Line<'static> {
    let prefix = if is_cursor { "  ▸ " } else { "    " };
    let mark = if multi {
        if is_selected { "[x] " } else { "[ ] " }
    } else if is_selected {
        "(•) "
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
    Line::from(Span::styled(format!("{prefix}{mark}{label}"), style))
}
