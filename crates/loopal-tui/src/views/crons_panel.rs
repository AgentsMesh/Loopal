//! Cron jobs panel — shows scheduled prompts with next-fire countdown.
//!
//! ```text
//!  ▸ ⏱ abc12345  Clean log cache                 next 2m 30s  [R]
//!    ⏱ def67890  One-shot reminder               next 14s
//! ```
//!
//! Rendered below the BgTasks panel. Data sourced from `App.cron_snapshots`
//! (synced from `SessionState.cron_snapshots` via `CronsChanged` events).

use chrono::Utc;
use loopal_protocol::CronJobSnapshot;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::cron_duration_format::format_next_fire_ms;
use super::text_width::{display_width, truncate_to_width};

pub const MAX_CRON_VISIBLE: usize = 4;
const CRON_ICON: &str = "⏱";

pub fn crons_panel_height(snapshots: &[CronJobSnapshot]) -> u16 {
    if snapshots.is_empty() {
        return 0;
    }
    snapshots.len().min(MAX_CRON_VISIBLE) as u16
}

pub fn cron_ids(snapshots: &[CronJobSnapshot]) -> Vec<String> {
    snapshots.iter().map(|s| s.id.clone()).collect()
}

pub fn render_crons_panel(
    f: &mut Frame,
    snapshots: &[CronJobSnapshot],
    focused: Option<&str>,
    offset: usize,
    area: Rect,
) {
    if area.height == 0 || snapshots.is_empty() {
        return;
    }
    let total = snapshots.len();
    let clamped = offset.min(total.saturating_sub(MAX_CRON_VISIBLE));
    let end = (clamped + MAX_CRON_VISIBLE).min(total);
    let now = Utc::now();
    let lines: Vec<Line<'static>> = snapshots[clamped..end]
        .iter()
        .map(|c| render_cron_line(c, focused, now, area.width as usize))
        .collect();
    let bg = Style::default().bg(Color::Rgb(25, 25, 30));
    f.render_widget(Paragraph::new(lines).style(bg), area);
}

fn render_cron_line(
    cron: &CronJobSnapshot,
    focused: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
    width: usize,
) -> Line<'static> {
    let is_focused = focused == Some(cron.id.as_str());
    let indicator = if is_focused { " ▸ " } else { "   " };
    let indicator_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default()
    };
    let id_label = format!("{:<9}", cron.id);
    let id_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let recurring_tag = if cron.recurring { "  [R]" } else { "" };
    let next_str = format_next_fire_ms(cron.next_fire_unix_ms, now);
    let suffix = format!("  next {next_str}{recurring_tag}");

    // Measure actual terminal width of the fixed prefix (indicator + icon +
    // space + id_label) rather than hardcoding a magic number.
    let prefix_width =
        display_width(indicator) + display_width(CRON_ICON) + 1 + display_width(&id_label);
    let suffix_width = display_width(&suffix);
    let max_prompt = width.saturating_sub(prefix_width + suffix_width + 1);
    let (prompt, prompt_width) = truncate_to_width(&cron.prompt, max_prompt);
    let pad = max_prompt.saturating_sub(prompt_width);

    Line::from(vec![
        Span::styled(indicator.to_string(), indicator_style),
        Span::styled(CRON_ICON.to_string(), Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        Span::styled(id_label, id_style),
        Span::styled(prompt, Style::default().fg(Color::White)),
        Span::raw(" ".repeat(pad)),
        Span::styled(suffix, Style::default().fg(Color::Rgb(80, 80, 80))),
    ])
}
