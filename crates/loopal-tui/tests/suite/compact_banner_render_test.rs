use loopal_tui::views::compact_progress::{banner_height, render_compact_banner};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn render(message: &str, width: u16) -> String {
    let backend = TestBackend::new(width, 1);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_compact_banner(f, message, Rect::new(0, 0, width, 1));
    })
    .unwrap();
    term.backend().to_string()
}

#[test]
fn banner_height_is_zero_when_absent() {
    assert_eq!(banner_height(&None), 0);
}

#[test]
fn banner_height_is_one_when_present() {
    assert_eq!(banner_height(&Some("anything".into())), 1);
}

#[test]
fn render_emits_phase_label_text() {
    let out = render("⠙ summarizing context", 60);
    assert!(
        out.contains("summarizing context"),
        "phase label missing from frame: {out}",
    );
}

#[test]
fn render_emits_marker_prefix() {
    let out = render("anything", 40);
    assert!(out.contains("▼"), "marker prefix missing: {out}");
}

#[test]
fn render_does_nothing_for_zero_sized_area() {
    let backend = TestBackend::new(20, 1);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_compact_banner(f, "x", Rect::new(0, 0, 0, 0));
    })
    .unwrap();
    let out = term.backend().to_string();
    assert!(
        !out.contains("x"),
        "zero-sized area must short-circuit before any draw: {out}",
    );
}

#[test]
fn render_truncates_to_area_width() {
    let out = render(
        "long banner text that exceeds tight terminals horizontally",
        20,
    );
    let lines: Vec<&str> = out.lines().collect();
    let first = lines.first().expect("at least one line");
    assert!(
        first.chars().count() <= 22,
        "rendered line longer than the 20-cell area: {first:?}",
    );
}
