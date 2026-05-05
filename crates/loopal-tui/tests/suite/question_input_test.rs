use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{ControlCommand, Question, QuestionOption, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::{InputAction, handle_key};
use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn install_question(app: &mut App, opts: Vec<&str>, multi: bool) {
    let options = opts
        .into_iter()
        .map(|l| QuestionOption {
            label: l.into(),
            description: String::new(),
        })
        .collect();
    app.with_active_conversation_mut(|conv| {
        conv.pending_question = Some(loopal_view_state::PendingQuestion::new(
            "q1".into(),
            vec![Question {
                question: "?".into(),
                options,
                allow_multiple: multi,
            }],
        ));
    });
}

fn set_cursor(app: &mut App, c: usize) {
    app.with_active_conversation_mut(|conv| {
        if let Some(q) = conv.pending_question.as_mut()
            && let Some(s) = q.states.get_mut(q.current_question)
        {
            s.cursor = c;
        }
    });
}

#[test]
fn char_on_option_row_is_ignored() {
    let mut app = make_app();
    install_question(&mut app, vec!["A", "B"], false);
    set_cursor(&mut app, 0);
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::None));
}

#[test]
fn char_on_other_row_returns_free_text_char() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::QuestionFreeTextChar('x')));
}

#[test]
fn empty_options_routes_chars_as_free_text() {
    let mut app = make_app();
    install_question(&mut app, vec![], false);
    let action = handle_key(&mut app, key(KeyCode::Char('a')));
    assert!(matches!(action, InputAction::QuestionFreeTextChar('a')));
}

#[test]
fn space_on_option_row_toggles() {
    let mut app = make_app();
    install_question(&mut app, vec!["A", "B"], true);
    set_cursor(&mut app, 0);
    let action = handle_key(&mut app, key(KeyCode::Char(' ')));
    assert!(matches!(action, InputAction::QuestionToggle));
}

#[test]
fn space_on_other_row_inserts_space() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    let action = handle_key(&mut app, key(KeyCode::Char(' ')));
    assert!(matches!(action, InputAction::QuestionFreeTextChar(' ')));
}

#[test]
fn backspace_on_other_row_deletes() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    let action = handle_key(&mut app, key(KeyCode::Backspace));
    assert!(matches!(action, InputAction::QuestionFreeTextBackspace));
}

#[test]
fn backspace_on_option_row_is_ignored() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 0);
    let action = handle_key(&mut app, key(KeyCode::Backspace));
    assert!(matches!(action, InputAction::None));
}

#[test]
fn left_right_on_other_row_moves_cursor() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Left)),
        InputAction::QuestionFreeTextCursorLeft
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Right)),
        InputAction::QuestionFreeTextCursorRight
    ));
}

#[test]
fn enter_always_confirms() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 0);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)),
        InputAction::QuestionConfirm
    ));
    set_cursor(&mut app, 1);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)),
        InputAction::QuestionConfirm
    ));
}

#[test]
fn delete_on_other_routes_to_free_text_delete() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    let action = handle_key(&mut app, key(KeyCode::Delete));
    assert!(matches!(action, InputAction::QuestionFreeTextDelete));
}

#[test]
fn home_end_on_other_routes_to_cursor_jump() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Home)),
        InputAction::QuestionFreeTextHome
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::End)),
        InputAction::QuestionFreeTextEnd
    ));
}

#[test]
fn home_end_on_option_row_ignored() {
    let mut app = make_app();
    install_question(&mut app, vec!["A", "B"], false);
    set_cursor(&mut app, 0);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Home)),
        InputAction::None
    ));
}

#[test]
fn ctrl_v_routes_to_paste_requested() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    let key = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL);
    let action = handle_key(&mut app, key);
    assert!(matches!(action, InputAction::PasteRequested));
}
