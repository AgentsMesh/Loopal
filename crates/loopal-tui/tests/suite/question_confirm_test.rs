use loopal_protocol::{Question, QuestionOption};
use loopal_tui::dispatch_ops::compute_question_answers;
use loopal_view_state::PendingQuestion;

fn opt(label: &str) -> QuestionOption {
    QuestionOption {
        label: label.into(),
        description: String::new(),
    }
}

fn make(opts: Vec<QuestionOption>, multi: bool) -> PendingQuestion {
    PendingQuestion::new(
        "q1".into(),
        vec![Question {
            question: "Q?".into(),
            options: opts,
            allow_multiple: multi,
        }],
    )
}

fn answers(q: &PendingQuestion) -> Vec<String> {
    compute_question_answers(q)
}

fn set_cursor(q: &mut PendingQuestion, c: usize) {
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = c;
        s.interacted = true;
    }
}

#[test]
fn empty_options_no_free_text_returns_empty_string() {
    let q = make(vec![], false);
    assert_eq!(answers(&q), vec![String::new()]);
}

#[test]
fn empty_options_with_free_text_returns_text() {
    let mut q = make(vec![], false);
    "hello".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["hello".to_string()]);
}

#[test]
fn single_select_cursor_on_real_option_returns_label() {
    let mut q = make(vec![opt("A"), opt("B"), opt("C")], false);
    set_cursor(&mut q, 1);
    assert_eq!(answers(&q), vec!["B".to_string()]);
}

#[test]
fn single_select_cursor_on_other_with_free_text_returns_text() {
    let mut q = make(vec![opt("A"), opt("B")], false);
    set_cursor(&mut q, 2);
    "custom".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["custom".to_string()]);
}

#[test]
fn single_select_cursor_on_other_whitespace_only_returns_empty_string() {
    let mut q = make(vec![opt("A")], false);
    set_cursor(&mut q, 1);
    "   \t".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec![String::new()]);
}

#[test]
fn single_select_free_text_is_trimmed() {
    let mut q = make(vec![opt("A")], false);
    set_cursor(&mut q, 1);
    "  hi  ".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["hi".to_string()]);
}

#[test]
fn single_select_toggle_is_noop_does_not_affect_answer() {
    let mut q = make(vec![opt("A"), opt("B")], false);
    set_cursor(&mut q, 0);
    q.toggle();
    set_cursor(&mut q, 1);
    assert_eq!(answers(&q), vec!["B".to_string()]);
}

#[test]
fn multi_select_no_selection_returns_empty_string() {
    let q = make(vec![opt("A"), opt("B")], true);
    assert_eq!(answers(&q), vec![String::new()]);
}

#[test]
fn multi_select_joins_selected_labels_with_comma() {
    let mut q = make(vec![opt("A"), opt("B"), opt("C")], true);
    set_cursor(&mut q, 0);
    q.toggle();
    set_cursor(&mut q, 2);
    q.toggle();
    assert_eq!(answers(&q), vec!["A, C".to_string()]);
}

#[test]
fn multi_select_combines_selected_with_free_text_when_other_toggled() {
    let mut q = make(vec![opt("A"), opt("B")], true);
    set_cursor(&mut q, 0);
    q.toggle();
    set_cursor(&mut q, 2);
    q.toggle();
    "extra".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["A, extra".to_string()]);
}

#[test]
fn multi_select_free_text_without_other_toggled_is_dropped() {
    let mut q = make(vec![opt("A"), opt("B")], true);
    set_cursor(&mut q, 0);
    q.toggle();
    "extra".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["A".to_string()]);
}

#[test]
fn multi_select_only_free_text_when_other_toggled() {
    let mut q = make(vec![opt("A")], true);
    set_cursor(&mut q, 1);
    q.toggle();
    "only".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["only".to_string()]);
}

#[test]
fn multi_select_other_toggled_empty_text_returns_empty_string() {
    let mut q = make(vec![opt("A")], true);
    set_cursor(&mut q, 1);
    q.toggle();
    assert_eq!(answers(&q), vec![String::new()]);
}

#[test]
fn cursor_out_of_range_returns_empty_string() {
    let mut q = make(vec![opt("A")], false);
    set_cursor(&mut q, 99);
    assert_eq!(answers(&q), vec![String::new()]);
}

#[test]
fn multi_question_returns_one_answer_per_question() {
    let mut q = PendingQuestion::new(
        "q".into(),
        vec![
            Question {
                question: "Q1".into(),
                options: vec![opt("A"), opt("B")],
                allow_multiple: false,
            },
            Question {
                question: "Q2".into(),
                options: vec![],
                allow_multiple: false,
            },
        ],
    );
    set_cursor(&mut q, 1);
    q.advance_to_next();
    "second".chars().for_each(|c| q.free_text_insert_char(c));
    assert_eq!(answers(&q), vec!["B".to_string(), "second".to_string()]);
}
