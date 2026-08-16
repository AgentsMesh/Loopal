use std::sync::Arc;
use std::time::{Duration, Instant};

use loopal_protocol::{
    AgentEvent, AgentEventPayload, AgentStatus, ControlCommand, ProjectedMessage, SubAgentSpawn,
    UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use tokio::sync::{mpsc, watch};

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(8);
    let (permission_tx, _) = mpsc::channel::<bool>(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, _) = watch::channel(0_u64);
    App::new(
        SessionController::new(
            control_tx,
            permission_tx,
            question_tx,
            Default::default(),
            Arc::new(interrupt_tx),
        ),
        std::env::temp_dir(),
    )
}

fn projected(role: &str, content: &str) -> ProjectedMessage {
    ProjectedMessage {
        role: role.into(),
        content: content.into(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
    }
}

#[test]
fn transient_status_obeys_clear_and_expiry_windows_without_waiting() {
    let mut app = make_app();
    assert_eq!(app.current_transient_status(), None);

    app.set_transient_status("saved");
    app.clear_transient_status();
    assert_eq!(app.current_transient_status(), Some("saved"));

    app.transient_status.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(2);
    app.clear_transient_status();
    assert_eq!(app.current_transient_status(), None);

    app.set_transient_status("expired");
    app.transient_status.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(4);
    assert_eq!(app.current_transient_status(), None);
}

#[test]
fn welcome_is_local_only_and_does_not_interrupt_a_real_conversation() {
    let mut app = make_app();
    app.push_welcome("model-a", "/workspace");
    let messages = app.snapshot_conversation("main").messages;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "welcome");
    assert_eq!(messages[0].content, "model-a\n/workspace");
    assert!(messages[0].ui_local);

    app.view_clients["main"].with_conversation_mut(|conversation| {
        conversation
            .messages
            .push(loopal_view_state::SessionMessage {
                role: "user".into(),
                content: "already started".into(),
                ..Default::default()
            });
    });
    app.push_welcome("model-b", "/other");
    assert_eq!(app.snapshot_conversation("main").messages.len(), 2);

    app.view_clients.shift_remove("main");
    app.push_welcome("model-c", "/missing");
}

#[test]
fn display_history_is_replaced_and_marked_ui_local() {
    let app = make_app();
    app.load_display_history(vec![
        projected("user", "one"),
        projected("assistant", "two"),
    ]);

    let conversation = app.snapshot_conversation("main");
    assert_eq!(conversation.messages.len(), 2);
    assert_eq!(conversation.messages[0].content, "one");
    assert_eq!(conversation.messages[1].content, "two");
    assert!(conversation.messages.iter().all(|message| message.ui_local));
}

#[test]
fn sub_agent_history_creates_then_updates_the_same_view() {
    let mut app = make_app();
    app.load_sub_agent_history(
        "worker",
        "session-1",
        Some("main"),
        Some("model-a"),
        vec![projected("assistant", "first")],
    );

    {
        let state = app.view_clients["worker"].state();
        let agent = &state.state().agent;
        assert_eq!(agent.session_id.as_deref(), Some("session-1"));
        assert_eq!(agent.parent.as_deref(), Some("main"));
        assert_eq!(agent.observable.model, "model-a");
        assert_eq!(agent.observable.status, AgentStatus::Finished);
        assert!(agent.conversation.messages[0].ui_local);
    }

    app.load_sub_agent_history(
        "worker",
        "session-2",
        None,
        None,
        vec![projected("user", "second")],
    );
    let state = app.view_clients["worker"].state();
    let agent = &state.state().agent;
    assert_eq!(agent.session_id.as_deref(), Some("session-2"));
    assert_eq!(agent.parent, None);
    assert_eq!(agent.observable.model, "model-a");
    assert_eq!(agent.conversation.messages[0].content, "second");
}

#[test]
fn spawn_seeds_optional_parent_only_when_the_view_is_created() {
    let mut app = make_app();
    let spawned = SubAgentSpawn {
        name: "worker".into(),
        agent_id: "agent-1".into(),
        parent: Some("main".into()),
        model: None,
        session_id: None,
    };
    assert!(!app.dispatch_event(AgentEvent::named(
        "worker",
        AgentEventPayload::SubAgentSpawned(spawned.clone()),
    )));
    assert_eq!(
        app.view_clients["worker"]
            .state()
            .state()
            .agent
            .parent
            .as_deref(),
        Some("main")
    );

    let duplicate = SubAgentSpawn {
        parent: None,
        ..spawned
    };
    app.dispatch_event(AgentEvent::named(
        "worker",
        AgentEventPayload::SubAgentSpawned(duplicate),
    ));
    assert_eq!(
        app.view_clients["worker"]
            .state()
            .state()
            .agent
            .parent
            .as_deref(),
        Some("main")
    );

    app.dispatch_event(AgentEvent::named(
        "orphan",
        AgentEventPayload::SubAgentSpawned(SubAgentSpawn {
            name: "orphan".into(),
            agent_id: "agent-2".into(),
            parent: None,
            model: None,
            session_id: None,
        }),
    ));
    assert_eq!(
        app.view_clients["orphan"].state().state().agent.parent,
        None
    );
}
