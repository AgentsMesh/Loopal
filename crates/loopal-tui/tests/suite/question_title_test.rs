use loopal_protocol::{Question, QuestionOption};
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

fn q(question: &str, header: Option<&str>) -> Question {
    Question {
        question: question.into(),
        options: vec![opt("A"), opt("B")],
        allow_multiple: false,
        header: header.map(|s| s.into()),
    }
}

fn render(q: &PendingQuestion, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| {
        question_inline::render(f, q, Rect::new(0, 0, w, h), None);
    })
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
fn single_question_shows_only_question_text_no_chips() {
    let pq = PendingQuestion::new("id".into(), vec![q("Pick one", None)]);
    let s = render(&pq, 60, 8);
    assert!(s.contains("? Pick one"), "buffer:\n{s}");
    assert!(
        !s.contains("(1/1)"),
        "single question should not show counter"
    );
    assert!(!s.contains("[•"), "single question should not show chips");
}

#[test]
fn multi_question_shows_chip_row_with_counter() {
    let pq = PendingQuestion::new(
        "id".into(),
        vec![
            q("First question text", Some("Profile")),
            q("Second", Some("Replies")),
            q("Third", Some("Render")),
        ],
    );
    let s = render(&pq, 80, 10);
    assert!(
        s.contains("[•Profile]"),
        "active chip with marker missing:\n{s}"
    );
    assert!(s.contains("[Replies]"));
    assert!(s.contains("[Render]"));
    assert!(s.contains("(1/3)"));
    assert!(s.contains("? First question text"));
}

#[test]
fn switching_current_question_moves_chip_highlight() {
    let mut pq = PendingQuestion::new(
        "id".into(),
        vec![
            q("Q1", Some("Alpha")),
            q("Q2", Some("Beta")),
            q("Q3", Some("Gamma")),
        ],
    );
    pq.next_question();
    let s = render(&pq, 80, 10);
    assert!(s.contains("[Alpha]"));
    assert!(s.contains("[•Beta]"));
    assert!(s.contains("[Gamma]"));
    assert!(s.contains("(2/3)"));
}

#[test]
fn missing_header_falls_back_to_truncated_question_text() {
    let pq = PendingQuestion::new(
        "id".into(),
        vec![
            q(
                "This is a fairly long question that should be truncated",
                None,
            ),
            q("Short", None),
        ],
    );
    let s = render(&pq, 80, 10);
    assert!(
        s.contains("[•This is a fairly"),
        "chip should show truncated question prefix:\n{s}"
    );
    assert!(s.contains("…"), "truncation ellipsis missing");
    assert!(s.contains("[Short]"));
}

#[test]
fn multi_question_hint_includes_switch_shortcut() {
    let pq = PendingQuestion::new("id".into(), vec![q("A", Some("X")), q("B", Some("Y"))]);
    let s = render(&pq, 80, 10);
    assert!(
        s.contains("←→ Switch"),
        "multi-question hint missing ←→:\n{s}"
    );
}

#[test]
fn single_question_hint_does_not_show_switch() {
    let pq = PendingQuestion::new("id".into(), vec![q("Only", None)]);
    let s = render(&pq, 60, 8);
    assert!(!s.contains("←→ Switch"));
    assert!(s.contains("⏎ Submit"));
}

#[test]
fn empty_question_string_falls_back_to_question_mark() {
    let pq = PendingQuestion::new("id".into(), vec![q("", None)]);
    let s = render(&pq, 60, 8);
    assert!(
        s.contains("? ?"),
        "empty question should fallback to '?':\n{s}"
    );
}

#[test]
fn header_takes_precedence_over_question_for_chip_label() {
    let pq = PendingQuestion::new(
        "id".into(),
        vec![
            q("Long question text not used for chip", Some("Short hdr")),
            q("Other", Some("Hdr2")),
        ],
    );
    let s = render(&pq, 80, 10);
    assert!(s.contains("[•Short hdr]"));
    assert!(!s.contains("[•Long question"));
}
