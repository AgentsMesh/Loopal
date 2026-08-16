use std::sync::Arc;

use loopal_protocol::{
    AgentMode, ControlCommand, SkillInvocation, UserContent, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::CommandEffect;
use loopal_tui::dispatch_ops::handle_effect;
use tokio::sync::{mpsc, watch};

fn make_app() -> (App, mpsc::Receiver<ControlCommand>) {
    let (control_tx, control_rx) = mpsc::channel(16);
    let (permission_tx, _) = mpsc::channel::<bool>(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, _) = watch::channel(0_u64);
    let session = SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        Default::default(),
        Arc::new(interrupt_tx),
    );
    (App::new(session, std::env::temp_dir()), control_rx)
}

#[tokio::test]
async fn done_reply_quit_and_detach_apply_their_documented_effects() {
    let (mut app, _) = make_app();
    assert!(!handle_effect(&mut app, CommandEffect::Done).await);

    assert!(!handle_effect(&mut app, CommandEffect::Reply("notice".into())).await);
    let messages = app.snapshot_active_conversation().messages;
    assert_eq!(messages.last().unwrap().content, "notice");
    assert_eq!(messages.last().unwrap().role, "system");
    assert!(messages.last().unwrap().ui_local);

    assert!(handle_effect(&mut app, CommandEffect::Quit).await);
    assert!(app.exiting);

    let (mut detached, _) = make_app();
    assert!(handle_effect(&mut detached, CommandEffect::Detach).await);
    assert!(detached.detach_requested);
    assert!(detached.exiting);
}

#[tokio::test]
async fn inbox_history_uses_plain_text_or_the_skill_invocation() {
    let (mut app, _) = make_app();
    app.history_index = Some(3);

    for content in [
        UserContent::text_only("plain"),
        UserContent {
            text: "expanded body".into(),
            images: Vec::new(),
            skill_info: Some(SkillInvocation {
                name: "/review".into(),
                user_args: String::new(),
            }),
        },
        UserContent {
            text: "expanded body".into(),
            images: Vec::new(),
            skill_info: Some(SkillInvocation {
                name: "/review".into(),
                user_args: "src/lib.rs".into(),
            }),
        },
    ] {
        assert!(!handle_effect(&mut app, CommandEffect::InboxPush(content)).await);
    }

    assert_eq!(
        app.input_history,
        vec!["plain", "/review", "/review src/lib.rs"]
    );
    assert_eq!(app.history_index, None);
}

#[tokio::test]
async fn mode_switch_and_resume_are_forwarded_to_the_control_plane() {
    let (mut app, mut controls) = make_app();
    assert!(!handle_effect(&mut app, CommandEffect::ModeSwitch(AgentMode::Plan)).await);
    assert!(matches!(
        controls.try_recv(),
        Ok(ControlCommand::ModeSwitch(AgentMode::Plan))
    ));

    assert!(!handle_effect(&mut app, CommandEffect::ResumeSession("session-42".into()),).await);
    assert!(matches!(
        controls.try_recv(),
        Ok(ControlCommand::ResumeSession(id)) if id == "session-42"
    ));
}
