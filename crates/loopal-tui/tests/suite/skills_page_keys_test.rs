/// Skills page key handling tests: Esc close, Up/Down navigation.
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, SkillItem, SkillsPageState, SubPage};
use loopal_tui::input::{InputAction, handle_key};

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".into(),
        "act".into(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, PathBuf::from("/tmp"))
}

fn open_skills_page(app: &mut App, count: usize) {
    let items: Vec<SkillItem> = (0..count)
        .map(|i| SkillItem {
            name: format!("/skill-{i}"),
            source: "project".into(),
            description: format!("Skill {i}"),
            has_arg: false,
        })
        .collect();
    app.sub_page = Some(SubPage::SkillsPage(SkillsPageState::new(items)));
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn selected(app: &App) -> usize {
    match app.sub_page.as_ref().unwrap() {
        SubPage::SkillsPage(s) => s.selected,
        _ => panic!("expected SkillsPage"),
    }
}

#[test]
fn test_esc_closes_skills_page() {
    let mut app = make_app();
    open_skills_page(&mut app, 3);
    assert!(app.sub_page.is_some());
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.sub_page.is_none());
}

#[test]
fn test_down_navigates() {
    let mut app = make_app();
    open_skills_page(&mut app, 3);
    assert_eq!(selected(&app), 0);
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(selected(&app), 1);
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(selected(&app), 2);
}

#[test]
fn test_down_clamps_at_end() {
    let mut app = make_app();
    open_skills_page(&mut app, 2);
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(selected(&app), 1);
}

#[test]
fn test_up_navigates() {
    let mut app = make_app();
    open_skills_page(&mut app, 3);
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(selected(&app), 2);
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(selected(&app), 1);
}

#[test]
fn test_up_clamps_at_start() {
    let mut app = make_app();
    open_skills_page(&mut app, 3);
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(selected(&app), 0);
}

#[test]
fn test_ctrl_c_closes_skills_page() {
    let mut app = make_app();
    open_skills_page(&mut app, 2);
    let action = handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
    assert!(matches!(action, InputAction::None));
    assert!(app.sub_page.is_none());
}
