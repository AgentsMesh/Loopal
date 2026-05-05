use loopal_protocol::{Question, QuestionOption, UserQuestionResponse};
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
fn height_below_min_renders_fallback() {
    let q = PendingQuestion::new("id".into(), vec![make_q(&["A"], false)]);
    let s = render_to_buffer(60, 2, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("Screen too small"), "fallback must show:\n{s}");
}

#[test]
fn unsupported_response_format_includes_reason() {
    let resp = UserQuestionResponse::unsupported("qid", "AskUser not supported in this context");
    let (formatted, is_error) =
        loopal_runtime::agent_loop::question_parse::format_response_for_test(&resp, &[]);
    assert_eq!(
        formatted,
        "(unsupported: AskUser not supported in this context)"
    );
    assert!(is_error, "unsupported should be is_error=true");
}

#[test]
fn cancelled_response_format_is_fixed_token() {
    let resp = UserQuestionResponse::cancelled("qid");
    let (formatted, is_error) =
        loopal_runtime::agent_loop::question_parse::format_response_for_test(&resp, &[]);
    assert_eq!(formatted, "(cancelled by user)");
    assert!(!is_error, "cancelled should be is_error=false");
}

#[test]
fn answered_response_protocol_mismatch_is_warned() {
    let resp = UserQuestionResponse::answered("qid", vec!["a".to_string(), "b".to_string()]);
    let (formatted, is_error) =
        loopal_runtime::agent_loop::question_parse::format_response_for_test(&resp, &[]);
    assert!(formatted.contains("protocol mismatch"));
    assert!(is_error, "protocol mismatch should be is_error=true");
}

#[test]
fn untouched_single_select_returns_empty_in_compute() {
    let q = PendingQuestion::new("id".into(), vec![make_q(&["A", "B"], false)]);
    let answers = compute_question_answers(&q);
    assert_eq!(
        answers,
        vec![String::new()],
        "untouched cursor must return empty answer to avoid silent confirmation"
    );
}

#[test]
fn touched_single_select_returns_cursor_label() {
    let mut q = PendingQuestion::new("id".into(), vec![make_q(&["A", "B"], false)]);
    q.cursor_down();
    q.cursor_up();
    let answers = compute_question_answers(&q);
    assert_eq!(answers, vec!["A".to_string()]);
}
