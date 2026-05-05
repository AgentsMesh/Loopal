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
async fn test_detach_hub_returns_detach_effect() {
    let mut app = make_app();
    assert!(!app.detach_requested);

    let handler = app
        .command_registry
        .find("/detach-hub")
        .expect("/detach-hub registered");
    let effect = handler.execute(&mut app, None).await;

    assert!(
        matches!(effect, CommandEffect::Detach),
        "must return Detach (not Quit) so handle_effect can branch"
    );
    assert!(
        !app.detach_requested,
        "the flag is owned by handle_effect, not the command"
    );
}

#[test]
fn test_detach_hub_is_builtin() {
    let app = make_app();
    let handler = app.command_registry.find("/detach-hub").unwrap();
    assert!(!handler.is_skill());
    assert_eq!(handler.name(), "/detach-hub");
}

#[test]
fn test_detach_hub_listed_in_entries() {
    let app = make_app();
    let names: Vec<String> = app
        .command_registry
        .entries()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(names.iter().any(|n| n == "/detach-hub"));
}
