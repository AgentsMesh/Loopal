use std::sync::Arc;

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, EnumPickerKind};
use loopal_tui::input::{InputAction, SubPageResult};
use loopal_tui::key_dispatch_for_test::dispatch;
use loopal_view_state::{PendingPermission, PermissionChoice};
use tokio::sync::{mpsc, watch};

struct TestApp {
    app: App,
    controls: mpsc::Receiver<ControlCommand>,
    permissions: mpsc::Receiver<bool>,
}

fn make_app() -> TestApp {
    let (control_tx, controls) = mpsc::channel(32);
    let (permission_tx, permissions) = mpsc::channel(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, _) = watch::channel(0_u64);
    TestApp {
        app: App::new(
            SessionController::new(
                control_tx,
                permission_tx,
                question_tx,
                Default::default(),
                Arc::new(interrupt_tx),
            ),
            std::env::temp_dir(),
        ),
        controls,
        permissions,
    }
}

fn seed_permission(app: &App, choice: PermissionChoice) {
    app.with_active_conversation_mut(|conversation| {
        conversation.pending_permission = Some(PendingPermission {
            id: "permission-1".into(),
            name: "Write".into(),
            input: serde_json::json!({"path": "a"}),
            intent_digest: None,
            cursor: choice,
        });
    });
}

#[tokio::test]
async fn permission_actions_are_noops_without_a_pending_request() {
    let mut test = make_app();
    for action in [
        InputAction::ToolApprove,
        InputAction::ToolDeny,
        InputAction::ToolPermissionToggle,
        InputAction::ToolPermissionConfirm,
    ] {
        dispatch(&mut test.app, action).await;
    }
    assert!(test.permissions.try_recv().is_err());
}

#[tokio::test]
async fn permission_toggle_and_confirm_follow_the_selected_choice() {
    let mut test = make_app();
    seed_permission(&test.app, PermissionChoice::Allow);
    dispatch(&mut test.app, InputAction::ToolPermissionToggle).await;
    dispatch(&mut test.app, InputAction::ToolPermissionConfirm).await;
    assert!(matches!(test.permissions.try_recv(), Ok(false)));

    seed_permission(&test.app, PermissionChoice::Deny);
    dispatch(&mut test.app, InputAction::ToolPermissionToggle).await;
    dispatch(&mut test.app, InputAction::ToolPermissionConfirm).await;
    assert!(matches!(test.permissions.try_recv(), Ok(true)));
}

#[tokio::test]
async fn picker_confirmations_forward_every_control_variant() {
    let mut test = make_app();
    let cases = [
        SubPageResult::ModelSelected("model-a".into()),
        SubPageResult::RewindConfirmed(7),
        SubPageResult::SessionSelected("session-a".into()),
        SubPageResult::EnumConfigSelected {
            kind: EnumPickerKind::Permission,
            value: "ask_any_write".into(),
        },
        SubPageResult::EnumConfigSelected {
            kind: EnumPickerKind::Decision,
            value: "classifier".into(),
        },
        SubPageResult::EnumConfigSelected {
            kind: EnumPickerKind::Sandbox,
            value: "read_only".into(),
        },
    ];
    for result in cases {
        dispatch(&mut test.app, InputAction::SubPageConfirm(result)).await;
    }

    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::ModelSwitch(v)) if v == "model-a")
    );
    assert!(matches!(
        test.controls.try_recv(),
        Ok(ControlCommand::Rewind { turn_index: 7 })
    ));
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::ResumeSession(v)) if v == "session-a")
    );
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::PermissionModeSwitch(v)) if v == "ask_any_write")
    );
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::DecisionModeSwitch(v)) if v == "classifier")
    );
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::SandboxPolicySwitch(v)) if v == "read_only")
    );
}

#[tokio::test]
async fn combined_model_thinking_and_mcp_actions_preserve_order_and_payloads() {
    let mut test = make_app();
    dispatch(
        &mut test.app,
        InputAction::SubPageConfirm(SubPageResult::ModelAndThinkingSelected {
            model: "model-b".into(),
            thinking_json: "{\"type\":\"enabled\"}".into(),
        }),
    )
    .await;
    dispatch(&mut test.app, InputAction::McpReconnect("docs".into())).await;
    dispatch(&mut test.app, InputAction::McpDisconnect("files".into())).await;

    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::ModelSwitch(v)) if v == "model-b")
    );
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::ThinkingSwitch(v)) if v.contains("enabled"))
    );
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::McpReconnect { server }) if server == "docs")
    );
    assert!(
        matches!(test.controls.try_recv(), Ok(ControlCommand::McpDisconnect { server }) if server == "files")
    );
}
