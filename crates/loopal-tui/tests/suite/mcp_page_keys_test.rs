use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use loopal_protocol::{McpServerSnapshot, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, McpPageState, SubPage};
use loopal_tui::input::{InputAction, handle_key};

fn make_app() -> App {
    let (control_tx, _) = tokio::sync::mpsc::channel(16);
    let (perm_tx, _) = tokio::sync::mpsc::channel::<bool>(16);
    let (question_tx, _) = tokio::sync::mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn servers() -> Vec<McpServerSnapshot> {
    vec![
        McpServerSnapshot {
            name: "a".into(),
            transport: "stdio".into(),
            source: "project".into(),
            status: "connected".into(),
            tool_count: 2,
            resource_count: 0,
            prompt_count: 0,
            errors: vec![],
        },
        McpServerSnapshot {
            name: "b".into(),
            transport: "streamable-http".into(),
            source: "global".into(),
            status: "failed: err".into(),
            tool_count: 0,
            resource_count: 0,
            prompt_count: 0,
            errors: vec!["err".into()],
        },
    ]
}

#[test]
fn test_esc_closes_page() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.sub_page.is_none());
}

#[test]
fn test_down_increments_selection() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("wrong"),
    };
    assert_eq!(s.selected, 1);
}

#[test]
fn test_up_at_zero_stays() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("wrong"),
    };
    assert_eq!(s.selected, 0);
}

#[test]
fn test_down_clamps_at_end() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    for _ in 0..10 {
        handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("wrong"),
    };
    assert_eq!(s.selected, 1);
}

#[test]
fn test_enter_opens_action_menu() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    let a = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(a, InputAction::None));
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("wrong"),
    };
    assert!(s.action_menu.is_some());
    assert_eq!(s.action_menu.as_ref().unwrap().server_name, "a");
}

#[test]
fn test_enter_on_second_item_opens_menu() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let a = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(a, InputAction::None));
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("wrong"),
    };
    assert_eq!(s.action_menu.as_ref().unwrap().server_name, "b");
}

#[test]
fn test_enter_empty_list_returns_none() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(vec![]))));
    let a = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(a, InputAction::None));
}

#[test]
fn test_ctrl_c_closes_page() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
    assert!(app.sub_page.is_none());
}

#[test]
fn test_unknown_key_noop() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    let a = handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert!(matches!(a, InputAction::None));
    assert!(app.sub_page.is_some());
}

#[test]
fn test_ctrl_d_returns_quit() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    let a = handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    );
    assert!(matches!(a, InputAction::Quit));
}
