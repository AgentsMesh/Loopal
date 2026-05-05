use loopal_protocol::{Question, QuestionOption};
use loopal_tui::dispatch_ops::compute_question_answers;
use loopal_tui::views::question_inline;
use loopal_view_state::PendingQuestion;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn opt(label: &str) -> QuestionOption {
    QuestionOption {
        label: label.into(),
        description: String::new(),
    }
}

fn make_q(opts: &[&str], multi: bool) -> Question {
    Question {
        question: "?".into(),
        options: opts.iter().map(|l| opt(l)).collect(),
        allow_multiple: multi,
    }
}

fn render_capture_cursor(
    width: u16,
    height: u16,
    draw: impl FnOnce(&mut ratatui::Frame, Rect),
) -> Option<(u16, u16)> {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| draw(f, Rect::new(0, 0, width, height)))
        .unwrap();
    term.get_cursor_position().ok().map(|p| (p.x, p.y))
}

fn render_to_buffer(
    width: u16,
    height: u16,
    draw: impl FnOnce(&mut ratatui::Frame, Rect),
) -> String {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| draw(f, Rect::new(0, 0, width, height)))
        .unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn cursor_in_middle_position_with_mixed_chars() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&[], false)]);
    "你Aあ".chars().for_each(|c| q.free_text_insert_char(c));
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.free_text_cursor = 1;
    }
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set");
    assert_eq!(pos.0, 6 + 2, "cursor col after one wide char: {pos:?}");
}

#[test]
fn cursor_at_zero_in_chinese_string() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&[], false)]);
    "你好".chars().for_each(|c| q.free_text_insert_char(c));
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.free_text_cursor = 0;
    }
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set");
    assert_eq!(pos.0, 6, "cursor col at start of chinese: {pos:?}");
}

#[test]
fn single_select_invariant_selection_stays_empty() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&["A", "B", "C"], false)]);
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = 1;
        s.interacted = true;
    }
    q.toggle();
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = 2;
        s.interacted = true;
    }
    q.toggle();
    assert!(
        q.selection().iter().all(|&b| !b),
        "single-select selection vector must remain all false"
    );
}

#[test]
fn multi_question_independent_state() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![make_q(&["A", "B"], true), make_q(&["X", "Y", "Z"], false)],
    );
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = 1;
        s.interacted = true;
    }
    q.toggle();
    "first".chars().for_each(|c| q.free_text_insert_char(c));

    q.advance_to_next();
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = 2;
        s.interacted = true;
    }

    let answers: Vec<String> = compute_question_answers(&q);
    assert_eq!(answers, vec!["B".to_string(), "Z".to_string()]);
}

#[test]
fn multi_question_question_2_of_3_title_format() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![
            make_q(&["A"], false),
            make_q(&["B"], false),
            make_q(&["C"], false),
        ],
    );
    q.advance_to_next();
    let s = render_to_buffer(60, 6, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("(2/3)"), "title should show 2/3, got:\n{s}");
}

#[test]
fn very_narrow_screen_keeps_hint_visible() {
    let q = PendingQuestion::new("id".into(), vec![make_q(&["A", "B", "C", "D", "E"], false)]);
    let s = render_to_buffer(60, 3, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("Esc Cancel"), "hint must be visible:\n{s}");
}

#[test]
fn options_window_around_cursor_in_narrow_area() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&["A", "B", "C", "D", "E"], false)]);
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = 4;
        s.interacted = true;
    }
    let s = render_to_buffer(60, 5, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("E"), "cursor option must be visible:\n{s}");
}

#[test]
fn cursor_at_last_char_position() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&[], false)]);
    "abc".chars().for_each(|c| q.free_text_insert_char(c));
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.free_text_cursor = 3;
    }
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set");
    assert_eq!(pos.0, 6 + 3);
}

#[test]
fn cursor_with_emoji() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&[], false)]);
    "ab🎉".chars().for_each(|c| q.free_text_insert_char(c));
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set");
    let prefix = 6u16;
    let typed = 2 + 2;
    assert_eq!(
        pos.0,
        prefix + typed,
        "emoji width=2: expected {}, got {pos:?}",
        prefix + typed
    );
}

#[test]
fn cursor_before_emoji() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&[], false)]);
    "ab🎉".chars().for_each(|c| q.free_text_insert_char(c));
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.free_text_cursor = 2;
    }
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set");
    assert_eq!(pos.0, 6 + 2, "cursor before emoji: expected 8, got {pos:?}");
}

#[test]
fn height_one_does_not_panic() {
    let q = PendingQuestion::new("id".into(), vec![make_q(&["A"], false)]);
    let _ = render_to_buffer(60, 1, |f, area| question_inline::render(f, &q, area, None));
}
