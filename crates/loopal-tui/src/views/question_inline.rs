use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_view_state::PendingQuestion;
use loopal_view_state::conversation::ClassifierStatus;

use super::question_inline_body::{render_main, title_line, wrapped_lines};

const MAX_HEIGHT: u16 = 12;
const MIN_HEIGHT: u16 = 3;

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
    let status_line: u16 = if q.classifier_status.is_none() { 0 } else { 1 };

    (question_lines + options_lines + other_line + free_text_line + hint_line + status_line)
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
    let status_widget = classifier_status_line(&q.classifier_status);
    let main_area = if status_widget.is_some() && area.height > MIN_HEIGHT {
        Rect {
            height: area.height.saturating_sub(1),
            ..area
        }
    } else {
        area
    };
    if let Some((widget, style)) = status_widget {
        let status_y = main_area.y + main_area.height;
        let status_area = Rect {
            x: area.x,
            y: status_y,
            width: area.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(widget, style))),
            status_area,
        );
    }
    render_main(f, q, main_area, status);
}

pub(crate) fn classifier_status_line(status: &ClassifierStatus) -> Option<(String, Style)> {
    match status {
        ClassifierStatus::None => None,
        ClassifierStatus::Running { elapsed_ms } => {
            let secs = *elapsed_ms as f32 / 1000.0;
            Some((
                format!("▶ Classifier: thinking... {secs:.1}s"),
                Style::default().fg(Color::Cyan).italic(),
            ))
        }
        ClassifierStatus::Failed { reason } => Some((
            format!("▶ Classifier: 失败 - {reason}"),
            Style::default().fg(Color::Red).bold(),
        )),
        ClassifierStatus::Completed { answers } => Some((
            format!("▶ Classifier: ✓ {}", answers.join(", ")),
            Style::default().fg(Color::Green).bold(),
        )),
    }
}
