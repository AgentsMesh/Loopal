//! Edge-case tests for crons_panel — focus, scroll, CJK, truncation.

use loopal_protocol::CronJobSnapshot;
use loopal_tui::views::crons_panel::render_crons_panel;
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
fn focused_row_has_pointer_indicator() {
    let crons = vec![
        snap("focus-me", "hello", true, Some(1_700_000_010_000)),
        snap("other", "bye", false, None),
    ];
    let text = render_to_text(&crons, Some("focus-me"), 0, 80, 2);
    // First row holds focused cron → pointer ▸ precedes it.
    let first_line = text.lines().next().unwrap();
    assert!(
        first_line.contains('▸'),
        "focused row must contain ▸, got: {first_line}"
    );
}

#[test]
fn unfocused_row_lacks_pointer() {
    let crons = vec![snap("lonely", "p", false, None)];
    let text = render_to_text(&crons, None, 0, 80, 1);
    assert!(!text.contains('▸'), "unfocused panel must not show ▸");
}

#[test]
fn scroll_offset_skips_leading_entries() {
    let crons: Vec<_> = (0..8)
        .map(|i| snap(&format!("id{i:03}"), &format!("prompt-{i}"), true, None))
        .collect();
    let text = render_to_text(&crons, None, 3, 80, 4);
    // offset=3 → visible ids start at id003.
    assert!(text.contains("id003"), "offset=3 must start at id003");
    assert!(
        !text.contains("id000"),
        "offset=3 must not include id000/id001/id002"
    );
}

#[test]
fn long_prompt_is_truncated_within_width() {
    let long_prompt = "a".repeat(200);
    let crons = vec![snap("long", &long_prompt, false, Some(1_700_000_000_000))];
    let text = render_to_text(&crons, None, 0, 40, 1);
    // Panel is 40 cols wide; first row must not exceed 40 cells.
    let first_line = text.lines().next().unwrap();
    let visible = first_line.trim_end();
    assert!(
        visible.chars().count() <= 40,
        "truncation must keep row ≤ width, got len {}",
        visible.chars().count()
    );
    // Suffix "next" must still be present even under heavy truncation.
    assert!(first_line.contains("next"));
}

#[test]
fn cjk_prompt_renders_with_suffix_intact() {
    // CJK chars are 2 cells wide. Verify the suffix ("next …") survives
    // truncation — i.e. prefix + suffix layout math is correct.
    let crons = vec![snap(
        "cjk01",
        "清理日志缓存并重启服务",
        true,
        Some(1_700_000_030_000),
    )];
    let text = render_to_text(&crons, Some("cjk01"), 0, 60, 1);
    assert!(text.contains("next"), "suffix must survive CJK truncation");
    assert!(text.contains("[R]"), "[R] tag must survive CJK truncation");
}

#[test]
fn cjk_prompt_narrow_area_does_not_overflow() {
    // Extremely narrow width where the Japanese prompt must be heavily
    // truncated. Verify no panic and that ratatui didn't produce content
    // past column 30.
    let crons = vec![snap("n", "日本語の長いプロンプト", true, None)];
    let text = render_to_text(&crons, None, 0, 30, 1);
    let first_line = text.lines().next().unwrap();
    // Each character in TestBackend is one cell of output; the line is
    // truncated to exactly `width` cells.
    assert_eq!(first_line.chars().count(), 30);
}

#[test]
fn extremely_narrow_width_drops_prompt_but_keeps_suffix() {
    // width < prefix (14) + suffix (~20) → max_prompt == 0, prompt dropped.
    let crons = vec![snap(
        "n1",
        "some prompt text",
        true,
        Some(1_700_000_060_000),
    )];
    let text = render_to_text(&crons, None, 0, 25, 1);
    let first = text.lines().next().unwrap();
    // Must still render without panic, preserving id and the "next" suffix
    // even though the prompt can't fit.
    assert!(first.contains("n1"), "id must survive: {first}");
    assert!(
        first.contains("next") || first.contains("[R]"),
        "suffix must survive: {first}"
    );
    assert!(!first.contains("some prompt"), "prompt must be omitted");
}

#[test]
fn none_next_fire_renders_em_dash() {
    // CronJobSnapshot with next_fire_unix_ms = None → format_next_fire_ms
    // returns "—" which must appear in the rendered suffix.
    let crons = vec![snap("no-fire", "prompt", false, None)];
    let text = render_to_text(&crons, None, 0, 80, 1);
    assert!(
        text.contains("—"),
        "buffer must contain em-dash for None next_fire, got: {text}"
    );
}
