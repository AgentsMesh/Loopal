use ratatui::prelude::*;

use loopal_protocol::Question;
use loopal_view_state::PendingQuestion;

use super::question_inline_body::wrapped_lines;
use super::text_width::display_width;

const CHIP_GAP: &str = "  ";
const CHIP_MAX_LEN: usize = 16;

fn title_text(cur: &Question) -> String {
    if cur.question.is_empty() {
        "?".to_string()
    } else {
        cur.question.clone()
    }
}

pub(super) fn title_lines(q: &PendingQuestion, width: u16) -> Vec<Line<'static>> {
    let Some(cur) = q.questions.get(q.current_question) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if q.questions.len() > 1 {
        lines.extend(chip_row(q, width));
    }
    lines.extend(wrap_styled(
        &format!("? {}", title_text(cur)),
        width,
        Style::default().fg(Color::Cyan).bold(),
    ));
    lines
}

fn wrap_styled(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    wrapped_lines(text, width)
        .into_iter()
        .map(|s| Line::from(Span::styled(s, style)))
        .collect()
}

fn chip_row(q: &PendingQuestion, width: u16) -> Vec<Line<'static>> {
    let mut packer = SegmentPacker::new((width as usize).max(1));
    for (i, qn) in q.questions.iter().enumerate() {
        let label = chip_label(qn, i);
        let chip = render_chip(&label, i == q.current_question);
        packer.push(chip);
    }
    let counter = format!(" ({}/{})", q.current_question + 1, q.questions.len());
    packer.push(vec![Span::styled(
        counter,
        Style::default().fg(Color::DarkGray).italic(),
    )]);
    packer.finish()
}

struct SegmentPacker {
    max_width: usize,
    lines: Vec<Vec<Span<'static>>>,
    current_width: usize,
}

impl SegmentPacker {
    fn new(max_width: usize) -> Self {
        Self {
            max_width,
            lines: vec![Vec::new()],
            current_width: 0,
        }
    }

    fn push(&mut self, segment: Vec<Span<'static>>) {
        let segment_w: usize = segment.iter().map(|s| display_width(&s.content)).sum();
        let needs_gap = !self.last_line().is_empty();
        let gap_w = if needs_gap {
            display_width(CHIP_GAP)
        } else {
            0
        };
        if needs_gap && self.current_width + gap_w + segment_w > self.max_width {
            self.lines.push(Vec::new());
            self.current_width = 0;
        }
        // reason: invariant — `lines` is non-empty (seeded in `new`, only ever pushed-to).
        let line = self.lines.last_mut().unwrap();
        if !line.is_empty() {
            line.push(Span::styled(CHIP_GAP, Style::default().fg(Color::DarkGray)));
            self.current_width += gap_w;
        }
        line.extend(segment);
        self.current_width += segment_w;
    }

    fn last_line(&self) -> &Vec<Span<'static>> {
        self.lines.last().unwrap()
    }

    fn finish(self) -> Vec<Line<'static>> {
        self.lines.into_iter().map(Line::from).collect()
    }
}

fn chip_label(q: &Question, idx: usize) -> String {
    if let Some(h) = q.header.as_ref().filter(|s| !s.is_empty()) {
        h.clone()
    } else if !q.question.is_empty() {
        truncate(&q.question, CHIP_MAX_LEN)
    } else {
        format!("Q{}", idx + 1)
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = chars.into_iter().take(max_chars).collect();
        out.push('…');
        out
    }
}

fn render_chip(label: &str, is_active: bool) -> Vec<Span<'static>> {
    if is_active {
        vec![Span::styled(
            format!("[•{label}]"),
            Style::default().fg(Color::Cyan).bold(),
        )]
    } else {
        vec![Span::styled(
            format!("[{label}]"),
            Style::default().fg(Color::DarkGray),
        )]
    }
}
