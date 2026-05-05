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
    }
}

fn set_cursor(q: &mut PendingQuestion, c: usize) {
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = c;
    }
}

#[test]
fn other_index_equals_options_len() {
    let q = PendingQuestion::new("id".into(), vec![q_with_opts(3, false)]);
    assert_eq!(q.other_index(), 3);
}

#[test]
fn other_index_is_zero_when_options_empty() {
    let q = PendingQuestion::new("id".into(), vec![q_with_opts(0, false)]);
    assert_eq!(q.other_index(), 0);
    assert!(q.cursor_on_other());
}

#[test]
fn cursor_down_can_reach_other_row() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, false)]);
    q.cursor_down();
    assert_eq!(q.cursor(), 1);
    q.cursor_down();
    assert_eq!(q.cursor(), 2);
    assert!(q.cursor_on_other());
    q.cursor_down();
    assert_eq!(q.cursor(), 2, "cursor stops at other row");
}

#[test]
fn cursor_up_clamps_at_zero() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, false)]);
    q.cursor_up();
    assert_eq!(q.cursor(), 0);
}

#[test]
fn toggle_on_other_row_does_not_touch_selection() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, true)]);
    set_cursor(&mut q, 2);
    q.toggle();
    assert_eq!(q.selection(), &[false, false]);
}

#[test]
fn toggle_in_single_select_is_noop_on_selection() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, false)]);
    set_cursor(&mut q, 0);
    q.toggle();
    assert_eq!(q.selection(), &[false, false]);
}

#[test]
fn toggle_flips_selection_for_real_option_in_multi() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, true)]);
    set_cursor(&mut q, 1);
    q.toggle();
    assert_eq!(q.selection(), &[false, true]);
    q.toggle();
    assert_eq!(q.selection(), &[false, false]);
}

#[test]
fn get_answers_returns_selected_labels() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(3, true)]);
    set_cursor(&mut q, 0);
    q.toggle();
    set_cursor(&mut q, 2);
    q.toggle();
    assert_eq!(
        q.get_answers(),
        vec!["opt0".to_string(), "opt2".to_string()]
    );
}

#[test]
fn get_answers_safe_when_current_question_out_of_range() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, false)]);
    q.current_question = 99;
    assert!(q.get_answers().is_empty());
}

#[test]
fn allow_multiple_for_current_reads_active_question() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![q_with_opts(2, true), q_with_opts(2, false)],
    );
    assert!(q.allow_multiple_for_current());
    q.current_question = 1;
    assert!(!q.allow_multiple_for_current());
}

#[test]
fn allow_multiple_returns_false_when_current_question_out_of_range() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(2, true)]);
    q.current_question = 99;
    assert!(!q.allow_multiple_for_current());
}

#[test]
fn other_index_uses_current_question() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![q_with_opts(3, false), q_with_opts(1, true)],
    );
    assert_eq!(q.other_index(), 3);
    q.current_question = 1;
    assert_eq!(q.other_index(), 1);
}

#[test]
fn advance_to_next_returns_false_at_last_question() {
    let mut q = PendingQuestion::new("id".into(), vec![q_with_opts(1, false)]);
    assert!(!q.advance_to_next());
    assert_eq!(q.current_question, 0);
}

#[test]
fn advance_to_next_progresses_through_questions() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![
            q_with_opts(1, false),
            q_with_opts(2, true),
            q_with_opts(0, false),
        ],
    );
    assert!(q.advance_to_next());
    assert_eq!(q.current_question, 1);
    assert!(q.advance_to_next());
    assert_eq!(q.current_question, 2);
    assert!(!q.advance_to_next());
    assert_eq!(q.current_question, 2);
}

#[test]
fn per_question_state_is_independent() {
    let mut q = PendingQuestion::new(
        "id".into(),
        vec![q_with_opts(2, true), q_with_opts(3, true)],
    );
    set_cursor(&mut q, 0);
    q.toggle();
    "first".chars().for_each(|c| q.free_text_insert_char(c));

    q.advance_to_next();
    assert_eq!(q.cursor(), 0);
    assert_eq!(q.free_text(), "");
    assert_eq!(q.selection(), &[false, false, false]);

    q.current_question = 0;
    assert_eq!(q.free_text(), "first");
    assert_eq!(q.selection(), &[true, false]);
}
