//! Arrow-key navigation in the tool-permission dialog (regression: previously
//! only y/n typed letters worked; users couldn't browse Allow/Deny with arrow
//! keys when the permission prompt fired e.g. on EnterPlanMode).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::{InputAction, handle_key};
use loopal_view_state::{PendingPermission, PermissionChoice};
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

fn seed_permission(app: &App) {
    app.with_active_conversation_mut(|conv| {
        conv.pending_permission = Some(PendingPermission {
            id: "tool-1".into(),
            name: "EnterPlanMode".into(),
            input: serde_json::json!({}),
            intent_digest: None,
            cursor: PermissionChoice::Allow,
        });
    });
}

fn cursor_of(app: &App) -> PermissionChoice {
    app.with_active_conversation(|conv| {
        conv.pending_permission
            .as_ref()
            .map(|p| p.cursor)
            .unwrap_or(PermissionChoice::Allow)
    })
}

#[test]
fn arrow_right_toggles_permission_cursor_to_deny() {
    let mut app = make_app();
    seed_permission(&app);
    assert_eq!(cursor_of(&app), PermissionChoice::Allow);

    let action = handle_key(&mut app, key(KeyCode::Right));
    assert!(
        matches!(action, InputAction::ToolPermissionToggle),
        "Right must dispatch ToolPermissionToggle, got {action:?}"
    );
}

#[test]
fn arrow_left_toggles_permission_cursor() {
    let mut app = make_app();
    seed_permission(&app);

    let action = handle_key(&mut app, key(KeyCode::Left));
    assert!(matches!(action, InputAction::ToolPermissionToggle));
}

#[test]
fn arrow_up_down_toggle_permission_cursor() {
    let mut app = make_app();
    seed_permission(&app);
    for code in [KeyCode::Up, KeyCode::Down] {
        let action = handle_key(&mut app, key(code));
        assert!(
            matches!(action, InputAction::ToolPermissionToggle),
            "{code:?} must dispatch ToolPermissionToggle"
        );
    }
}

#[test]
fn tab_toggles_permission_cursor() {
    let mut app = make_app();
    seed_permission(&app);
    let action = handle_key(&mut app, key(KeyCode::Tab));
    assert!(matches!(action, InputAction::ToolPermissionToggle));
}

#[test]
fn enter_confirms_current_permission_cursor() {
    let mut app = make_app();
    seed_permission(&app);
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        matches!(action, InputAction::ToolPermissionConfirm),
        "Enter must dispatch ToolPermissionConfirm, got {action:?}"
    );
}

#[test]
fn y_n_shortcuts_still_work() {
    // Backward-compat: typing y/n must continue to dispatch immediately
    // without requiring users to learn the arrow-key flow.
    let mut app = make_app();
    seed_permission(&app);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Char('y'))),
        InputAction::ToolApprove
    ));

    seed_permission(&app);
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Char('n'))),
        InputAction::ToolDeny
    ));
}

#[test]
fn esc_still_denies() {
    let mut app = make_app();
    seed_permission(&app);
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(matches!(action, InputAction::ToolDeny));
}

#[test]
fn toggle_action_flips_cursor_value() {
    // The dispatch action mutates the cursor field on the pending permission.
    // After ToolPermissionToggle is applied, the cursor should be Deny.
    let mut app = make_app();
    seed_permission(&app);
    assert_eq!(cursor_of(&app), PermissionChoice::Allow);

    let action = handle_key(&mut app, key(KeyCode::Right));
    if let InputAction::ToolPermissionToggle = action {
        // Apply the side-effect manually (the input handler is pure;
        // dispatch is what mutates state).
        app.with_active_conversation_mut(|conv| {
            if let Some(p) = conv.pending_permission.as_mut() {
                p.cursor = p.cursor.toggle();
            }
        });
    }
    assert_eq!(cursor_of(&app), PermissionChoice::Deny);

    // Second toggle returns to Allow.
    let _ = handle_key(&mut app, key(KeyCode::Left));
    app.with_active_conversation_mut(|conv| {
        if let Some(p) = conv.pending_permission.as_mut() {
            p.cursor = p.cursor.toggle();
        }
    });
    assert_eq!(cursor_of(&app), PermissionChoice::Allow);
}
