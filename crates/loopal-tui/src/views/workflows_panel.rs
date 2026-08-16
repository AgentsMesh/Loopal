//! Root workflow panel backed by the workflow projection in `ViewState`.

use loopal_protocol::{WorkflowRunState, WorkflowRunSummary, WorkflowRunsSnapshot};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::text_width::{display_width, truncate_to_width};

pub const MAX_WORKFLOW_VISIBLE: usize = 5;

pub fn workflows_panel_height(snapshot: &WorkflowRunsSnapshot) -> u16 {
    workflow_count(snapshot).min(MAX_WORKFLOW_VISIBLE) as u16
}

pub fn workflow_ids(snapshot: &WorkflowRunsSnapshot) -> Vec<String> {
    runs(snapshot)
        .map(|run| run.id.as_str().to_owned())
        .collect()
}

pub fn render_workflows_panel(
    f: &mut Frame,
    snapshot: &WorkflowRunsSnapshot,
    focused: Option<&str>,
    offset: usize,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }
    let items: Vec<_> = runs(snapshot).collect();
    let clamped = offset.min(items.len().saturating_sub(MAX_WORKFLOW_VISIBLE));
    let end = (clamped + MAX_WORKFLOW_VISIBLE).min(items.len());
    let lines = items[clamped..end]
        .iter()
        .map(|run| render_workflow_line(run, focused, area.width as usize))
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(Color::Rgb(25, 25, 30))),
        area,
    );
}

fn render_workflow_line(
    run: &WorkflowRunSummary,
    focused: Option<&str>,
    width: usize,
) -> Line<'static> {
    let is_focused = focused == Some(run.id.as_str());
    let indicator = if is_focused { " > " } else { "   " };
    let indicator_style = if is_focused {
        Style::default().fg(Color::Cyan).bold()
    } else {
        Style::default()
    };
    let (state, state_style) = state_label(run.state);
    let terminal = [
        run.counts.succeeded,
        run.counts.failed,
        run.counts.cancelled,
        run.counts.skipped,
    ]
    .into_iter()
    .map(u64::from)
    .sum::<u64>();
    let total = terminal
        + u64::from(run.counts.pending)
        + u64::from(run.counts.ready)
        + u64::from(run.counts.active);
    let suffix = format!("  {state} {terminal}/{total}");
    let id_capacity = width
        .saturating_sub(display_width(indicator) + display_width(&suffix) + 1)
        .min(13);
    let (id_value, id_width) = truncate_to_width(run.id.as_str(), id_capacity);
    let id = format!(
        "{id_value}{}",
        " ".repeat(id_capacity.saturating_add(1).saturating_sub(id_width)),
    );
    let prefix_width = display_width(indicator) + display_width(&id);
    let suffix_width = display_width(&suffix);
    let goal_width = width.saturating_sub(prefix_width + suffix_width + 1);
    let (goal, rendered_width) = truncate_to_width(&run.run_goal, goal_width);

    Line::from(vec![
        Span::styled(indicator.to_owned(), indicator_style),
        Span::styled(id, Style::default().fg(Color::DarkGray)),
        Span::styled(goal, Style::default().fg(Color::White)),
        Span::raw(" ".repeat(goal_width.saturating_sub(rendered_width))),
        Span::styled(suffix, state_style),
    ])
}

fn runs(snapshot: &WorkflowRunsSnapshot) -> impl Iterator<Item = &WorkflowRunSummary> {
    snapshot.active.iter().chain(snapshot.recent.iter())
}

fn workflow_count(snapshot: &WorkflowRunsSnapshot) -> usize {
    snapshot.active.len() + snapshot.recent.len()
}

fn state_label(state: WorkflowRunState) -> (&'static str, Style) {
    match state {
        WorkflowRunState::Planned => ("planned", Style::default().fg(Color::DarkGray)),
        WorkflowRunState::Validated => ("ready", Style::default().fg(Color::Yellow)),
        WorkflowRunState::Running => ("running", Style::default().fg(Color::Cyan)),
        WorkflowRunState::Cancelling => ("stopping", Style::default().fg(Color::Yellow)),
        WorkflowRunState::Succeeded => ("done", Style::default().fg(Color::Green)),
        WorkflowRunState::Failed => ("failed", Style::default().fg(Color::Red)),
        WorkflowRunState::Cancelled => ("cancelled", Style::default().fg(Color::DarkGray)),
    }
}
