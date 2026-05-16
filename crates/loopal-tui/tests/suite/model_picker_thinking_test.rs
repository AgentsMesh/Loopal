use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use loopal_protocol::ControlCommand;
use loopal_session::SessionController;
use loopal_tui::app::{App, PickerState, SubPage, ThinkingOption};
use loopal_tui::input::handle_key;
use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (q_tx, _) = mpsc::channel::<loopal_protocol::UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        q_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn picker_with_five_thinking() -> PickerState {
    PickerState {
        title: "Switch Model".into(),
        items: vec![],
        filter: String::new(),
        filter_cursor: 0,
        selected: 0,
        thinking_options: vec![
            ThinkingOption { label: "Auto", value: r#"{"type":"auto"}"#.into() },
            ThinkingOption { label: "Low", value: r#"{"type":"effort","level":"low"}"#.into() },
            ThinkingOption { label: "Medium", value: r#"{"type":"effort","level":"medium"}"#.into() },
            ThinkingOption { label: "High", value: r#"{"type":"effort","level":"high"}"#.into() },
            ThinkingOption { label: "Disabled", value: r#"{"type":"disabled"}"#.into() },
        ],
        thinking_selected: 0,
    }
}

fn read_thinking_selected(app: &App) -> usize {
    match app.sub_page.as_ref().expect("sub_page must be set") {
        SubPage::ModelPicker(p) => p.thinking_selected,
        _ => panic!("expected ModelPicker"),
    }
}

#[test]
fn right_arrow_advances_thinking_selected() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::ModelPicker(picker_with_five_thinking()));
    handle_key(&mut app, key(KeyCode::Right));
    assert_eq!(read_thinking_selected(&app), 1, "Right must advance");
}

#[test]
fn left_arrow_wraps_to_last() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::ModelPicker(picker_with_five_thinking()));
    handle_key(&mut app, key(KeyCode::Left));
    assert_eq!(read_thinking_selected(&app), 4, "Left from 0 must wrap to last");
}

fn key_release(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    }
}

#[test]
fn release_event_must_not_advance_thinking_selected() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::ModelPicker(picker_with_five_thinking()));
    handle_key(&mut app, key_release(KeyCode::Right));
    assert_eq!(
        read_thinking_selected(&app),
        0,
        "Release events must be ignored — otherwise terminals that report key-release (kitty protocol, Windows console) will double-fire and either cancel out or step by 2"
    );
}

#[test]
fn release_char_event_must_not_be_inserted_into_filter() {
    let mut app = make_app();
    let mut p = picker_with_five_thinking();
    p.items = vec![loopal_tui::app::PickerItem {
        label: "claude".into(),
        description: "".into(),
        value: "claude".into(),
    }];
    app.sub_page = Some(SubPage::ModelPicker(p));
    handle_key(&mut app, key_release(KeyCode::Char('a')));
    let filter_len = match app.sub_page.as_ref().expect("sub_page") {
        SubPage::ModelPicker(p) => p.filter.len(),
        _ => panic!("expected ModelPicker"),
    };
    assert_eq!(
        filter_len, 0,
        "Release Char events must not be inserted as filter input"
    );
}

#[test]
fn filter_chars_do_not_affect_thinking_selected() {
    let mut app = make_app();
    let mut p = picker_with_five_thinking();
    p.items = vec![loopal_tui::app::PickerItem {
        label: "claude".into(),
        description: "".into(),
        value: "claude".into(),
    }];
    app.sub_page = Some(SubPage::ModelPicker(p));
    handle_key(&mut app, key(KeyCode::Char('c')));
    assert_eq!(read_thinking_selected(&app), 0, "typing filter must not move thinking");
}
