//! Collapsed skill invocation rendering for the progress view.
use loopal_protocol::SkillInvocation;
use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use crate::text_util::truncate_to_width;

/// Render a skill invocation as a single collapsed line: `▎ ▸ /name  [args]`.
pub(super) fn render_skill_invoke(
    lines: &mut Vec<Line<'static>>,
    skill: &SkillInvocation,
    width: u16,
) {
    let w = (width as usize).max(1);
    let bg = Color::Rgb(30, 35, 48);
    let accent = Style::default().fg(Color::Rgb(100, 130, 200)).bg(bg);
    let name_style = Style::default().fg(Color::Rgb(130, 180, 240)).bg(bg).bold();
    let args_style = Style::default().fg(Color::Rgb(155, 160, 175)).bg(bg);

    let prefix = "▎ ";
    let prefix_w = 3;
    let arrow = "▸ ";
    let arrow_w = 3; // ▸(2) + space(1)
    let name_w = UnicodeWidthStr::width(skill.name.as_str());

    let mut spans = vec![
        Span::styled(prefix.to_string(), accent),
        Span::styled(arrow.to_string(), name_style),
        Span::styled(skill.name.clone(), name_style),
    ];

    let used = prefix_w + arrow_w + name_w;
    if !skill.user_args.is_empty() {
        let sep = "  ";
        let budget = w.saturating_sub(used + sep.len());
        let truncated = truncate_to_width(&skill.user_args, budget);
        let truncated_w = UnicodeWidthStr::width(truncated.as_str());
        spans.push(Span::styled(sep.to_string(), args_style));
        spans.push(Span::styled(truncated, args_style));
        let pad = w.saturating_sub(used + sep.len() + truncated_w);
        spans.push(Span::styled(" ".repeat(pad), args_style));
    } else {
        let pad = w.saturating_sub(used);
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
    }

    lines.push(Line::from(spans));
}
