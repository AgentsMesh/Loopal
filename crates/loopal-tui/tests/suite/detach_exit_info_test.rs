use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, HubReconnectInfo};
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

#[test]
fn test_app_starts_without_reconnect_info_or_detach_intent() {
    let app = make_app();
    assert!(app.hub_reconnect_info.is_none());
    assert!(!app.detach_requested);
}

#[test]
fn test_bootstrap_can_inject_reconnect_info() {
    let mut app = make_app();
    app.hub_reconnect_info = Some(HubReconnectInfo {
        addr: "127.0.0.1:54321".into(),
        token: "abc123".into(),
    });
    let info = app.hub_reconnect_info.as_ref().unwrap();
    assert_eq!(info.addr, "127.0.0.1:54321");
    assert_eq!(info.token, "abc123");
}

#[tokio::test]
async fn test_detach_command_then_exit_info_is_loud() {
    let mut app = make_app();
    app.hub_reconnect_info = Some(HubReconnectInfo {
        addr: "127.0.0.1:7777".into(),
        token: "tok".into(),
    });

    let handler = app.command_registry.find("/detach-hub").unwrap();
    let effect = handler.execute(&mut app, None).await;

    let _quit = loopal_tui::dispatch_ops::handle_effect(&mut app, effect).await;
    assert!(app.detach_requested);
    assert!(app.exiting);
    let info = app.hub_reconnect_info.as_ref().unwrap();
    assert_eq!(info.addr, "127.0.0.1:7777");
    assert_eq!(info.token, "tok");
}

#[tokio::test]
async fn test_kill_command_keeps_exit_info_quiet() {
    let mut app = make_app();
    app.hub_reconnect_info = Some(HubReconnectInfo {
        addr: "127.0.0.1:7777".into(),
        token: "tok".into(),
    });

    let handler = app.command_registry.find("/kill-hub").unwrap();
    handler.execute(&mut app, None).await;

    assert!(
        !app.detach_requested,
        "post-kill exit must not surface re-attach instructions"
    );
}
