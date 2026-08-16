use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, Question, QuestionOption, ResolveSource,
    UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::InputAction;
use loopal_tui::key_dispatch_for_test;
use tokio::sync::mpsc;

struct TestApp {
    app: App,
    permission_rx: mpsc::Receiver<bool>,
    question_rx: mpsc::Receiver<UserQuestionResponse>,
}

fn make_app() -> TestApp {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, permission_rx) = mpsc::channel::<bool>(16);
    let (question_tx, question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    TestApp {
        app: App::new(session, std::env::temp_dir()),
        permission_rx,
        question_rx,
    }
}

fn request_permission(app: &mut App, id: &str) {
    app.dispatch_event(AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
        id: id.into(),
        name: "Write".into(),
        input: serde_json::json!({"file_path": "/tmp/example"}),
        permission_intent: None,
    }));
}

fn resolve_permission(app: &mut App, id: &str) {
    app.dispatch_event(AgentEvent::root(
        AgentEventPayload::ToolPermissionResolved { id: id.into() },
    ));
}

fn request_question(app: &mut App, id: &str) {
    app.dispatch_event(AgentEvent::root(AgentEventPayload::UserQuestionRequest {
        id: id.into(),
        logical_id: id.into(),
        questions: vec![Question {
            question: "Continue?".into(),
            options: vec![QuestionOption {
                label: "Yes".into(),
                description: String::new(),
            }],
            allow_multiple: false,
            header: None,
        }],
        classifier_running: false,
    }));
}

fn resolve_question(app: &mut App, id: &str) {
    app.dispatch_event(AgentEvent::root(AgentEventPayload::UserQuestionResolved {
        id: id.into(),
        by: ResolveSource::Manual,
    }));
}

fn pending_permission_id(app: &App) -> Option<String> {
    app.with_active_conversation(|conv| conv.pending_permission.as_ref().map(|p| p.id.clone()))
}

fn pending_question_id(app: &App) -> Option<String> {
    app.with_active_conversation(|conv| conv.pending_question.as_ref().map(|q| q.id.clone()))
}

#[tokio::test]
async fn permission_approve_waits_for_matching_resolved_event() {
    let mut test = make_app();
    request_permission(&mut test.app, "permission-1");

    key_dispatch_for_test::dispatch(&mut test.app, InputAction::ToolApprove).await;

    assert_eq!(test.permission_rx.recv().await, Some(true));
    assert_eq!(
        pending_permission_id(&test.app).as_deref(),
        Some("permission-1")
    );
    resolve_permission(&mut test.app, "stale-permission");
    assert_eq!(
        pending_permission_id(&test.app).as_deref(),
        Some("permission-1")
    );
    resolve_permission(&mut test.app, "permission-1");
    assert_eq!(pending_permission_id(&test.app), None);
}

#[tokio::test]
async fn permission_deny_waits_for_resolved_event() {
    let mut test = make_app();
    request_permission(&mut test.app, "permission-2");

    key_dispatch_for_test::dispatch(&mut test.app, InputAction::ToolDeny).await;

    assert_eq!(test.permission_rx.recv().await, Some(false));
    assert_eq!(
        pending_permission_id(&test.app).as_deref(),
        Some("permission-2")
    );
    resolve_permission(&mut test.app, "permission-2");
    assert_eq!(pending_permission_id(&test.app), None);
}

#[tokio::test]
async fn question_confirm_waits_for_matching_resolved_event() {
    let mut test = make_app();
    request_question(&mut test.app, "question-1");

    key_dispatch_for_test::dispatch(&mut test.app, InputAction::QuestionConfirm).await;

    assert_eq!(
        test.question_rx.recv().await,
        Some(UserQuestionResponse::answered(
            "question-1",
            vec!["Yes".into()]
        ))
    );
    assert_eq!(
        pending_question_id(&test.app).as_deref(),
        Some("question-1")
    );
    resolve_question(&mut test.app, "stale-question");
    assert_eq!(
        pending_question_id(&test.app).as_deref(),
        Some("question-1")
    );
    resolve_question(&mut test.app, "question-1");
    assert_eq!(pending_question_id(&test.app), None);
}

#[tokio::test]
async fn question_cancel_waits_for_resolved_event() {
    let mut test = make_app();
    request_question(&mut test.app, "question-2");

    key_dispatch_for_test::dispatch(&mut test.app, InputAction::QuestionCancel).await;

    assert_eq!(
        test.question_rx.recv().await,
        Some(UserQuestionResponse::cancelled("question-2"))
    );
    assert_eq!(
        pending_question_id(&test.app).as_deref(),
        Some("question-2")
    );
    resolve_question(&mut test.app, "question-2");
    assert_eq!(pending_question_id(&test.app), None);
}
