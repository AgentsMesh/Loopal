use std::sync::Arc;
use std::time::{Duration, Instant};

use loopal_protocol::{ControlCommand, PermissionIntentDigest, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::InputAction;
use loopal_tui::key_dispatch_for_test::dispatch;
use loopal_view_state::{PendingPermission, PermissionChoice};
use tokio::sync::{mpsc, watch};

fn app() -> (App, mpsc::Receiver<bool>) {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(8);
    let (permission_tx, permission_rx) = mpsc::channel(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, _) = watch::channel(0_u64);
    (
        App::new(
            SessionController::new(
                control_tx,
                permission_tx,
                question_tx,
                Default::default(),
                Arc::new(interrupt_tx),
            ),
            std::env::temp_dir(),
        ),
        permission_rx,
    )
}

fn pending(id: &str) -> PendingPermission {
    PendingPermission {
        id: id.into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "true"}),
        intent_digest: Some(PermissionIntentDigest::from_bytes([7; 32])),
        cursor: PermissionChoice::Allow,
    }
}

#[tokio::test]
async fn direct_permission_actions_forward_decisions_and_clear_old_status() {
    let (mut app, mut decisions) = app();
    app.set_transient_status("old failure");
    app.transient_status.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(2);
    app.with_active_conversation_mut(|conversation| {
        conversation.pending_permission = Some(pending("allow"));
    });

    dispatch(&mut app, InputAction::ToolApprove).await;
    assert!(decisions.try_recv().unwrap());
    assert_eq!(app.current_transient_status(), None);

    app.with_active_conversation_mut(|conversation| {
        conversation.pending_permission = Some(pending("deny"));
    });
    dispatch(&mut app, InputAction::ToolDeny).await;
    assert!(!decisions.try_recv().unwrap());
}
