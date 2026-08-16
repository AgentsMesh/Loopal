use std::sync::Arc;

use loopal_protocol::{
    ControlCommand, InterruptSignal, PermissionIntentDigest, UserContent, UserQuestionResponse,
};
use loopal_session::SessionController;
use tokio::sync::{mpsc, watch};

struct LocalFixture {
    controller: SessionController,
    control_rx: mpsc::Receiver<ControlCommand>,
    permission_rx: mpsc::Receiver<bool>,
    question_rx: mpsc::Receiver<UserQuestionResponse>,
    interrupt: InterruptSignal,
    interrupt_rx: watch::Receiver<u64>,
}

fn fixture() -> LocalFixture {
    let (control_tx, control_rx) = mpsc::channel(16);
    let (permission_tx, permission_rx) = mpsc::channel(16);
    let (question_tx, question_rx) = mpsc::channel(16);
    let (interrupt_tx, interrupt_rx) = watch::channel(0);
    let interrupt = InterruptSignal::new();
    let controller = SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        interrupt.clone(),
        Arc::new(interrupt_tx),
    );
    LocalFixture {
        controller,
        control_rx,
        permission_rx,
        question_rx,
        interrupt,
        interrupt_rx,
    }
}

#[test]
fn local_interrupts_signal_and_increment_generation() {
    let mut local = fixture();
    assert!(local.controller.enter_agent_view("child"));
    local.controller.interrupt();
    assert!(local.interrupt.take());
    assert_eq!(*local.interrupt_rx.borrow_and_update(), 1);

    local.controller.interrupt_agent("other");
    assert!(local.interrupt.is_signaled());
    assert_eq!(*local.interrupt_rx.borrow_and_update(), 2);
}

#[tokio::test]
async fn local_interaction_responses_use_channels() {
    let mut local = fixture();
    let digest = PermissionIntentDigest::from_bytes([0x5a; 32]);
    local
        .controller
        .respond_permission("main", "permission-1", Some(digest), true)
        .await;
    assert_eq!(local.permission_rx.recv().await, Some(true));

    local
        .controller
        .respond_question("main", "question-1", vec!["answer".into()])
        .await;
    assert_eq!(
        local.question_rx.recv().await,
        Some(UserQuestionResponse::answered(
            "question-1",
            vec!["answer".into()]
        ))
    );
    local.controller.cancel_question("main", "question-2").await;
    assert_eq!(
        local.question_rx.recv().await,
        Some(UserQuestionResponse::cancelled("question-2"))
    );

    local
        .controller
        .respond_plan_approval("main", "plan-1", true)
        .await;
    assert!(local.question_rx.try_recv().is_err());
}

#[tokio::test]
async fn local_control_variants_and_resume_are_forwarded() {
    let mut local = fixture();
    local.controller.enter_agent_view("child");
    local.controller.switch_thinking("high".into()).await;
    local
        .controller
        .switch_permission_mode("ask_any_write".into())
        .await;
    local
        .controller
        .switch_decision_mode("classifier".into())
        .await;
    local
        .controller
        .switch_sandbox_policy("read_only".into())
        .await;
    local.controller.rewind(4).await;
    local.controller.resume_session("session-2").await;

    assert!(matches!(
        local.control_rx.recv().await,
        Some(ControlCommand::ThinkingSwitch(value)) if value == "high"
    ));
    assert!(matches!(
        local.control_rx.recv().await,
        Some(ControlCommand::PermissionModeSwitch(value)) if value == "ask_any_write"
    ));
    assert!(matches!(
        local.control_rx.recv().await,
        Some(ControlCommand::DecisionModeSwitch(value)) if value == "classifier"
    ));
    assert!(matches!(
        local.control_rx.recv().await,
        Some(ControlCommand::SandboxPolicySwitch(value)) if value == "read_only"
    ));
    assert!(matches!(
        local.control_rx.recv().await,
        Some(ControlCommand::Rewind { turn_index: 4 })
    ));
    assert!(matches!(
        local.control_rx.recv().await,
        Some(ControlCommand::ResumeSession(value)) if value == "session-2"
    ));
    assert_eq!(local.controller.root_session_id().as_deref(), None);
}

#[tokio::test]
async fn local_queries_route_and_closed_channels_are_safe() {
    let local = fixture();
    assert!(local.controller.fetch_agent_names().await.is_empty());
    assert!(local.controller.list_agents().await.is_empty());
    assert_eq!(
        local
            .controller
            .fetch_view_snapshot("main")
            .await
            .unwrap_err(),
        "not in hub mode"
    );
    local
        .controller
        .route_message(UserContent::text_only("local route"))
        .await;
    local.controller.set_root_session_id("root-session");
    assert_eq!(
        local.controller.root_session_id().as_deref(),
        Some("root-session")
    );

    let LocalFixture {
        controller,
        control_rx,
        permission_rx,
        question_rx,
        ..
    } = fixture();
    drop((control_rx, permission_rx, question_rx));
    controller
        .send_control("main".into(), ControlCommand::Clear)
        .await;
    controller
        .respond_permission("main", "permission", None, false)
        .await;
    controller
        .respond_question("main", "question", vec![])
        .await;
    controller.cancel_question("main", "question").await;
}
