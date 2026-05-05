use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::CommandEffect;
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

#[tokio::test]
async fn test_kill_hub_returns_quit_without_marking_detach() {
    let mut app = make_app();
    assert!(!app.detach_requested);

    let handler = app
        .command_registry
        .find("/kill-hub")
        .expect("/kill-hub registered");
    let effect = handler.execute(&mut app, None).await;

    assert!(matches!(effect, CommandEffect::Quit));
    assert!(
        !app.detach_requested,
        "kill-hub must NOT set detach_requested (full shutdown is not detach)"
    );
}

#[test]
fn test_kill_hub_is_builtin() {
    let app = make_app();
    let handler = app.command_registry.find("/kill-hub").unwrap();
    assert!(!handler.is_skill());
    assert_eq!(handler.name(), "/kill-hub");
}

#[tokio::test]
async fn test_exit_returns_quit_without_marking_detach() {
    let mut app = make_app();
    let handler = app.command_registry.find("/exit").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(effect, CommandEffect::Quit));
    assert!(!app.detach_requested);
}
