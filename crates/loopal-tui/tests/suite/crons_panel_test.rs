//! Tests for crons_panel rendering and height computation.

use loopal_protocol::CronJobSnapshot;
use loopal_tui::views::crons_panel::{
    MAX_CRON_VISIBLE, cron_ids, crons_panel_height, render_crons_panel,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;

fn snap(id: &str, prompt: &str, recurring: bool, next_ms: Option<i64>) -> CronJobSnapshot {
    CronJobSnapshot {
        id: id.into(),
        cron_expr: "*/5 * * * *".into(),
        prompt: prompt.into(),
        recurring,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: next_ms,
        durable: false,
    }
}

/// Render the panel into a TestBackend and return the concatenated buffer
/// text (one row per newline). Enables content assertions instead of the
/// weaker "does not panic" checks.
fn render_to_text(
    crons: &[CronJobSnapshot],
    focused: Option<&str>,
    offset: usize,
    width: u16,
    height: u16,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, width, height);
            render_crons_panel(f, crons, focused, offset, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            text.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
        }
        text.push('\n');
    }
    text
}

#[test]
fn height_zero_when_empty() {
    assert_eq!(crons_panel_height(&[]), 0);
}

#[test]
fn height_counts_all_snapshots() {
    let crons = vec![
        snap("a", "one", true, None),
        snap("b", "two", false, None),
        snap("c", "three", true, None),
    ];
    assert_eq!(crons_panel_height(&crons), 3);
}

#[test]
fn height_capped_at_max() {
    let crons: Vec<_> = (0..10)
        .map(|i| snap(&format!("id{i}"), "p", true, None))
        .collect();
    assert_eq!(crons_panel_height(&crons), MAX_CRON_VISIBLE as u16);
}

#[test]
fn cron_ids_returns_all() {
    let crons = vec![
        snap("alpha", "x", true, None),
        snap("beta", "y", false, None),
    ];
    assert_eq!(cron_ids(&crons), vec!["alpha", "beta"]);
}

#[test]
fn render_empty_area_is_noop() {
    let crons = vec![snap("a", "p", true, Some(1_700_000_000_000))];
    let text = render_to_text(&crons, None, 0, 80, 0);
    // Zero height: no rows produced.
    assert_eq!(text, String::new());
}

#[test]
fn render_empty_list_leaves_area_blank() {
    let text = render_to_text(&[], None, 0, 80, 1);
    // Empty list: bridge already emits CronsChanged but panel contributes
    // nothing to the buffer; row remains spaces (no panic, no artifacts).
    assert!(text.trim().is_empty());
}

#[test]
fn render_shows_cron_id_and_prompt() {
    let crons = vec![snap("alpha123", "build docs", true, None)];
    let text = render_to_text(&crons, None, 0, 80, 1);
    assert!(text.contains("alpha123"), "buffer must show cron id");
    assert!(text.contains("build docs"), "buffer must show prompt");
}

#[test]
fn render_shows_recurring_tag_for_recurring_crons() {
    let recurring = vec![snap("rec-id", "job", true, None)];
    let one_shot = vec![snap("one-id", "job", false, None)];
    let rec_text = render_to_text(&recurring, None, 0, 80, 1);
    let one_text = render_to_text(&one_shot, None, 0, 80, 1);
    assert!(rec_text.contains("[R]"), "recurring cron must show [R]");
    assert!(!one_text.contains("[R]"), "one-shot cron must NOT show [R]");
}

#[test]
fn render_shows_next_label_for_any_cron() {
    let crons = vec![snap("id1", "prompt", false, Some(1_700_000_000_000))];
    let text = render_to_text(&crons, None, 0, 80, 1);
    assert!(
        text.contains("next"),
        "suffix must include 'next' literal, got: {text}"
    );
}
