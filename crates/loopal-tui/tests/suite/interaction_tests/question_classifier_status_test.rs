use loopal_protocol::{Question, QuestionOption};
use loopal_tui::views::question_inline;
use loopal_view_state::PendingQuestion;
use loopal_view_state::conversation::ClassifierStatus;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn make_question() -> Question {
    Question {
        question: "Pick?".into(),
        options: vec![
            QuestionOption {
                label: "yes".into(),
                description: "".into(),
            },
            QuestionOption {
                label: "no".into(),
                description: "".into(),
            },
        ],
        allow_multiple: false,
        header: None,
    }
}

fn render_q(q: &PendingQuestion, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        question_inline::render(f, q, Rect::new(0, 0, width, height), None);
    })
    .unwrap();
    term.backend().to_string()
}

#[test]
fn none_status_does_not_render_classifier_line() {
    let q = PendingQuestion::new("q".into(), vec![make_question()]);
    assert!(q.classifier_status.is_none());
    let out = render_q(&q, 60, 10);
    assert!(
        !out.contains("Classifier:"),
        "classifier line must not be drawn: {out}"
    );
}

#[test]
fn running_status_renders_thinking_with_seconds() {
    let q = PendingQuestion::new("q".into(), vec![make_question()]).with_classifier_running(true);
    let mut q = q;
    q.classifier_status = ClassifierStatus::Running { elapsed_ms: 2300 };
    let out = render_q(&q, 60, 10);
    assert!(
        out.contains("Classifier: thinking"),
        "missing thinking label: {out}"
    );
    assert!(out.contains("2.3s"), "missing 2.3s elapsed: {out}");
}

#[test]
fn failed_status_renders_reason() {
    let mut q = PendingQuestion::new("q".into(), vec![make_question()]);
    q.classifier_status = ClassifierStatus::Failed {
        reason: "LLM timeout".into(),
    };
    let out = render_q(&q, 60, 10);
    assert!(out.contains("失败"), "missing 失败 marker: {out}");
    assert!(out.contains("LLM timeout"), "missing reason: {out}");
}

#[test]
fn completed_status_renders_answer_checkmark() {
    let mut q = PendingQuestion::new("q".into(), vec![make_question()]);
    q.classifier_status = ClassifierStatus::Completed {
        answers: vec!["yes".into()],
    };
    let out = render_q(&q, 60, 10);
    assert!(out.contains("✓"), "missing checkmark: {out}");
    assert!(out.contains("yes"), "missing answer: {out}");
}

#[test]
fn height_increases_when_classifier_status_active() {
    let plain = PendingQuestion::new("q".into(), vec![make_question()]);
    let h_plain = question_inline::height(&plain, 60);
    let mut running = plain.clone();
    running.classifier_status = ClassifierStatus::Running { elapsed_ms: 0 };
    let h_running = question_inline::height(&running, 60);
    assert!(
        h_running >= h_plain,
        "classifier line should not shrink layout (plain={h_plain}, running={h_running})"
    );
}
