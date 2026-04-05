//! Tests for append_user_display() — status preservation and image handling.

use loopal_protocol::UserQuestionResponse;
use loopal_protocol::{AgentStatus, ControlCommand, ImageAttachment, UserContent};
use loopal_session::SessionController;
use tokio::sync::mpsc;

fn make_controller() -> SessionController {
    let (control_tx, _control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _perm_rx) = mpsc::channel::<bool>(16);
    let (question_tx, _question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    )
}

#[tokio::test]
async fn test_append_user_display_preserves_idle_status() {
    let ctrl = make_controller();
    ctrl.lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::WaitingForInput;

    ctrl.append_user_display(&UserContent::from("hello"));
    assert!(ctrl.lock().is_active_agent_idle());
}

#[tokio::test]
async fn test_append_user_display_preserves_running_status() {
    let ctrl = make_controller();
    ctrl.lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::Running;

    ctrl.append_user_display(&UserContent::from("queued"));
    assert!(!ctrl.lock().is_active_agent_idle());
}

#[tokio::test]
async fn test_append_user_display_with_images() {
    let ctrl = make_controller();
    let content = UserContent {
        text: "check this".to_string(),
        images: vec![
            ImageAttachment {
                media_type: "image/png".to_string(),
                data: "AAAA".to_string(),
            },
            ImageAttachment {
                media_type: "image/jpeg".to_string(),
                data: "BBBB".to_string(),
            },
        ],
        skill_info: None,
    };
    ctrl.append_user_display(&content);

    let state = ctrl.lock();
    let conv = state.active_conversation();
    let msg = conv.messages.last().unwrap();
    assert!(msg.content.contains("[+2 image(s)]"));
    assert_eq!(msg.image_count, 2);
}

#[tokio::test]
async fn test_append_user_display_no_images() {
    let ctrl = make_controller();
    ctrl.append_user_display(&UserContent::from("just text"));

    let state = ctrl.lock();
    let conv = state.active_conversation();
    let msg = conv.messages.last().unwrap();
    assert!(!msg.content.contains("image"));
    assert_eq!(msg.image_count, 0);
}
