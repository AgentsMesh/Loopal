use loopal_protocol::{Question, QuestionOption};
use loopal_view_state::PendingQuestion;

fn empty_q() -> PendingQuestion {
    PendingQuestion::new(
        "id".into(),
        vec![Question {
            question: "?".into(),
            options: vec![],
            allow_multiple: false,
        }],
    )
}

fn typed(q: &mut PendingQuestion, s: &str) {
    s.chars().for_each(|c| q.free_text_insert_char(c));
}

fn make_q(opts_count: usize, multi: bool) -> PendingQuestion {
    PendingQuestion::new(
        "id".into(),
        vec![Question {
            question: "?".into(),
            options: (0..opts_count)
                .map(|i| QuestionOption {
                    label: format!("opt{i}"),
                    description: String::new(),
                })
                .collect(),
            allow_multiple: multi,
        }],
    )
}

fn set_cursor(q: &mut PendingQuestion, c: usize) {
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = c;
    }
}

fn set_free_text_cursor(q: &mut PendingQuestion, c: usize) {
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.free_text_cursor = c;
    }
}

#[test]
fn insert_appends_in_order() {
    let mut q = empty_q();
    typed(&mut q, "hi");
    assert_eq!(q.free_text(), "hi");
    assert_eq!(q.free_text_cursor(), 2);
}

#[test]
fn insert_at_middle() {
    let mut q = empty_q();
    q.free_text_insert_char('a');
    q.free_text_insert_char('c');
    set_free_text_cursor(&mut q, 1);
    q.free_text_insert_char('b');
    assert_eq!(q.free_text(), "abc");
    assert_eq!(q.free_text_cursor(), 2);
}

#[test]
fn backspace_removes_prev_char() {
    let mut q = empty_q();
    typed(&mut q, "ab");
    q.free_text_backspace();
    assert_eq!(q.free_text(), "a");
    assert_eq!(q.free_text_cursor(), 1);
}

#[test]
fn backspace_at_start_is_noop() {
    let mut q = empty_q();
    q.free_text_backspace();
    assert_eq!(q.free_text(), "");
    assert_eq!(q.free_text_cursor(), 0);
}

#[test]
fn handles_multibyte_chars() {
    let mut q = empty_q();
    typed(&mut q, "你好");
    assert_eq!(q.free_text(), "你好");
    assert_eq!(q.free_text_cursor(), 2);
    q.free_text_backspace();
    assert_eq!(q.free_text(), "你");
    assert_eq!(q.free_text_cursor(), 1);
}

#[test]
fn cursor_clamped_to_string_len() {
    let mut q = empty_q();
    q.free_text_insert_char('a');
    q.free_text_cursor_right();
    q.free_text_cursor_right();
    assert_eq!(q.free_text_cursor(), 1);
    q.free_text_cursor_left();
    q.free_text_cursor_left();
    assert_eq!(q.free_text_cursor(), 0);
}

#[test]
fn delete_removes_char_under_cursor() {
    let mut q = empty_q();
    typed(&mut q, "abc");
    set_free_text_cursor(&mut q, 1);
    q.free_text_delete();
    assert_eq!(q.free_text(), "ac");
    assert_eq!(q.free_text_cursor(), 1);
}

#[test]
fn delete_at_end_is_noop() {
    let mut q = empty_q();
    typed(&mut q, "ab");
    q.free_text_delete();
    assert_eq!(q.free_text(), "ab");
}

#[test]
fn home_end_jump() {
    let mut q = empty_q();
    typed(&mut q, "hello");
    q.free_text_cursor_home();
    assert_eq!(q.free_text_cursor(), 0);
    q.free_text_cursor_end();
    assert_eq!(q.free_text_cursor(), 5);
}

#[test]
fn other_selected_starts_false() {
    let q = make_q(2, true);
    assert!(!q.other_is_selected());
}

#[test]
fn toggle_on_other_in_multi_flips_other_selected() {
    let mut q = make_q(2, true);
    set_cursor(&mut q, 2);
    q.toggle();
    assert!(q.other_is_selected());
    q.toggle();
    assert!(!q.other_is_selected());
}

#[test]
fn toggle_on_other_in_single_does_not_flip_other_selected() {
    let mut q = make_q(2, false);
    set_cursor(&mut q, 2);
    q.toggle();
    assert!(!q.other_is_selected());
}
