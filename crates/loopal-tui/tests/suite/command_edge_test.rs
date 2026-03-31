/// Command edge cases: handler effects, skill expansion, sub-page open.
use loopal_config::Skill;
use loopal_protocol::{AgentMode, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".into(),
        "act".into(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

// ---------------------------------------------------------------------------
// Handler effects
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_plan_cmd_returns_mode_switch() {
    let mut app = make_app();
    let handler = app.command_registry.find("/plan").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(
        effect,
        loopal_tui::command::CommandEffect::ModeSwitch(AgentMode::Plan)
    ));
}

#[tokio::test]
async fn test_act_cmd_returns_mode_switch() {
    let mut app = make_app();
    let handler = app.command_registry.find("/act").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(
        effect,
        loopal_tui::command::CommandEffect::ModeSwitch(AgentMode::Act)
    ));
}

#[tokio::test]
async fn test_exit_cmd_returns_quit() {
    let mut app = make_app();
    let handler = app.command_registry.find("/exit").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(effect, loopal_tui::command::CommandEffect::Quit));
}

#[tokio::test]
async fn test_status_cmd_pushes_system_message() {
    let mut app = make_app();
    let handler = app.command_registry.find("/status").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(effect, loopal_tui::command::CommandEffect::Done));
    let state = app.session.lock();
    let last = state
        .active_conversation()
        .messages
        .last()
        .expect("expected a status message");
    assert!(last.content.contains("Model:"));
    assert!(last.content.contains("Mode:"));
}

#[tokio::test]
async fn test_help_cmd_shows_all_commands() {
    let mut app = make_app();
    let handler = app.command_registry.find("/help").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(effect, loopal_tui::command::CommandEffect::Done));
    let state = app.session.lock();
    let last = state
        .active_conversation()
        .messages
        .last()
        .expect("expected help message");
    assert!(last.content.contains("/clear"));
    assert!(last.content.contains("/model"));
    assert!(last.content.contains("Shortcuts:"));
}

#[tokio::test]
async fn test_model_cmd_opens_sub_page() {
    let mut app = make_app();
    assert!(app.sub_page.is_none());
    let handler = app.command_registry.find("/model").unwrap();
    handler.execute(&mut app, None).await;
    assert!(app.sub_page.is_some());
}

#[tokio::test]
async fn test_rewind_on_idle_opens_sub_page() {
    let mut app = make_app();
    {
        let mut state = app.session.lock();
        state.active_conversation_mut().agent_idle = true;
        state
            .active_conversation_mut()
            .messages
            .push(loopal_session::SessionMessage {
                role: "user".into(),
                content: "hello".into(),
                tool_calls: Vec::new(),
                image_count: 0,
                skill_info: None,
            });
    }
    let handler = app.command_registry.find("/rewind").unwrap();
    handler.execute(&mut app, None).await;
    assert!(app.sub_page.is_some());
}

#[tokio::test]
async fn test_rewind_on_busy_agent_shows_error() {
    let mut app = make_app();
    {
        app.session.lock().active_conversation_mut().agent_idle = false;
    }
    let handler = app.command_registry.find("/rewind").unwrap();
    handler.execute(&mut app, None).await;
    assert!(app.sub_page.is_none());
    let state = app.session.lock();
    let last = state.active_conversation().messages.last().unwrap();
    assert!(last.content.contains("Cannot rewind"));
}

// ---------------------------------------------------------------------------
// Skill expansion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skill_handler_returns_inbox_push() {
    let mut app = make_app();
    let skills = vec![Skill {
        name: "/commit".into(),
        description: "Commit".into(),
        has_arg: true,
        body: "Review: $ARGUMENTS".into(),
    }];
    app.command_registry.reload_skills(&skills);
    let handler = app.command_registry.find("/commit").unwrap();
    let effect = handler.execute(&mut app, Some("fix bug")).await;
    match effect {
        loopal_tui::command::CommandEffect::InboxPush(content) => {
            assert_eq!(content.text, "Review: fix bug");
        }
        _ => panic!("expected InboxPush"),
    }
}

#[tokio::test]
async fn test_skill_expand_no_arguments_placeholder() {
    let mut app = make_app();
    let skills = vec![Skill {
        name: "/greet".into(),
        description: "Greet".into(),
        has_arg: true,
        body: "Hello world".into(),
    }];
    app.command_registry.reload_skills(&skills);
    let handler = app.command_registry.find("/greet").unwrap();
    let effect = handler.execute(&mut app, Some("user")).await;
    match effect {
        loopal_tui::command::CommandEffect::InboxPush(content) => {
            assert_eq!(content.text, "Hello world\nuser");
        }
        _ => panic!("expected InboxPush"),
    }
}

#[tokio::test]
async fn test_skill_body_accessible_via_handler() {
    let mut app = make_app();
    let skills = vec![Skill {
        name: "/review".into(),
        description: "Review code".into(),
        has_arg: false,
        body: "Please review the changes".into(),
    }];
    app.command_registry.reload_skills(&skills);
    let handler = app.command_registry.find("/review").unwrap();
    assert!(handler.is_skill());
    assert_eq!(handler.skill_body(), Some("Please review the changes"));
}
