use loopal_protocol::{Question, QuestionOption};
use loopal_view_state::PendingQuestion;

fn opt(label: &str) -> QuestionOption {
    QuestionOption {
        label: label.into(),
        description: String::new(),
    }
}

fn q_with_opts(n: usize, multi: bool) -> Question {
    Question {
        question: "?".into(),
        options: (0..n).map(|i| opt(&format!("opt{i}"))).collect(),
        allow_multiple: multi,
        header: None,
    }
}

#[test]
fn next_question_returns_false_at_last_question() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(1, false)]);
    assert!(!q.next_question());
    assert_eq!(q.current_question, 0);
}

#[test]
fn next_question_progresses_through_questions() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![
            q_with_opts(1, false),
            q_with_opts(2, true),
            q_with_opts(0, false),
        ],
    );
    assert!(q.next_question());
    assert_eq!(q.current_question, 1);
    assert!(q.next_question());
    assert_eq!(q.current_question, 2);
    assert!(!q.next_question());
    assert_eq!(q.current_question, 2);
}

#[test]
fn prev_question_returns_false_at_first_question() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(1, false)]);
    assert!(!q.prev_question());
    assert_eq!(q.current_question, 0);
}

#[test]
fn prev_question_walks_back() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![
            q_with_opts(1, false),
            q_with_opts(2, true),
            q_with_opts(0, false),
        ],
    );
    q.next_question();
    q.next_question();
    assert_eq!(q.current_question, 2);
    assert!(q.prev_question());
    assert_eq!(q.current_question, 1);
    assert!(q.prev_question());
    assert_eq!(q.current_question, 0);
    assert!(!q.prev_question());
    assert_eq!(q.current_question, 0);
}
