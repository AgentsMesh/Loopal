use loopal_protocol::{ControlCommand, Question, QuestionOption, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::dispatch_ops::route_paste;
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

fn read_free_text(app: &App) -> String {
    app.with_active_conversation(|conv| {
        conv.pending_question
            .as_ref()
            .map(|q| q.free_text().to_string())
            .unwrap_or_default()
    })
}

#[test]
fn route_paste_appends_to_free_text_when_on_other() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    let routed = route_paste(&mut app, "你好world");
    assert!(routed);
    assert_eq!(read_free_text(&app), "你好world");
}

#[test]
fn route_paste_is_noop_when_off_other() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 0);
    let routed = route_paste(&mut app, "ignored");
    assert!(!routed);
}

#[test]
fn route_paste_is_noop_without_pending_question() {
    let mut app = make_app();
    let routed = route_paste(&mut app, "anything");
    assert!(!routed);
}

#[test]
fn route_paste_strips_newlines() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    route_paste(&mut app, "line1\nline2\rline3");
    assert_eq!(read_free_text(&app), "line1line2line3");
}

#[test]
fn route_paste_appends_to_existing_text() {
    let mut app = make_app();
    install_question(&mut app, vec![], false);
    app.with_active_conversation_mut(|conv| {
        if let Some(q) = conv.pending_question.as_mut() {
            "hi".chars().for_each(|c| q.free_text_insert_char(c));
        }
    });
    route_paste(&mut app, " world");
    assert_eq!(read_free_text(&app), "hi world");
}

#[test]
fn paste_text_falls_through_to_input_when_modal_off_other() {
    let mut app = make_app();
    install_question(&mut app, vec!["A", "B"], false);
    set_cursor(&mut app, 0);
    let routed = route_paste(&mut app, "fallthrough");
    assert!(!routed, "should not route when cursor is on option row");
    assert!(read_free_text(&app).is_empty());
}

#[test]
fn paste_with_modal_keeps_state_intact_when_user_moved_cursor() {
    let mut app = make_app();
    install_question(&mut app, vec!["A"], false);
    set_cursor(&mut app, 1);
    "x".chars().for_each(|c| {
        app.with_active_conversation_mut(|conv| {
            if let Some(q) = conv.pending_question.as_mut() {
                q.free_text_insert_char(c);
            }
        });
    });
    set_cursor(&mut app, 0);
    let routed = route_paste(&mut app, "asynced");
    assert!(!routed);
    set_cursor(&mut app, 1);
    assert_eq!(read_free_text(&app), "x", "free_text must not have grown");
}
