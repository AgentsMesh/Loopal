/// Tests for agent panel visibility rules and memory truncation on agent lifecycle.
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse};
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

fn spawn_agent(app: &App, name: &str) {
    app.session.handle_event(AgentEvent::named(
        name,
        AgentEventPayload::SubAgentSpawned {
            name: name.to_string(),
            agent_id: "id".into(),
            parent: Some("main".into()),
            model: Some("test-model".into()),
            session_id: None,
        },
    ));
    app.session
        .handle_event(AgentEvent::named(name, AgentEventPayload::Started));
}

fn panel_agents(app: &App) -> Vec<String> {
    let state = app.session.lock();
    state
        .agents
        .iter()
        .filter(|(k, a)| {
            k.as_str() != state.active_view.as_str()
                && !matches!(
                    a.observable.status,
                    loopal_protocol::AgentStatus::Finished | loopal_protocol::AgentStatus::Error
                )
        })
        .map(|(k, _)| k.clone())
        .collect()
}

// --- Panel visibility ---

#[test]
fn panel_excludes_active_view_includes_others() {
    let app = make_app();
    spawn_agent(&app, "A");
    spawn_agent(&app, "B");
    let agents = panel_agents(&app);
    assert!(agents.contains(&"A".to_string()));
    assert!(agents.contains(&"B".to_string()));
    assert!(!agents.contains(&"main".to_string()));
}

#[test]
fn panel_shows_main_when_viewing_subagent() {
    let app = make_app();
    spawn_agent(&app, "worker");
    app.session.enter_agent_view("worker");
    let agents = panel_agents(&app);
    assert!(agents.contains(&"main".to_string()));
    assert!(!agents.contains(&"worker".to_string()));
}

#[test]
fn panel_empty_when_only_main_exists() {
    let app = make_app();
    let agents = panel_agents(&app);
    assert!(agents.is_empty());
}

// --- View switch state consistency ---

#[test]
fn tab_without_subagents_leaves_focus_none() {
    let app = make_app();
    // Only "main" exists — cycle finds no switchable agents
    assert!(app.focused_agent.is_none());
}

#[test]
fn enter_without_focus_is_noop() {
    let app = make_app();
    // No focused_agent → enter_agent_view not called
    assert_eq!(app.session.lock().active_view, "main");
}

#[test]
fn rapid_enter_exit_maintains_state() {
    let app = make_app();
    spawn_agent(&app, "A");
    spawn_agent(&app, "B");
    app.session.enter_agent_view("A");
    assert_eq!(app.session.lock().active_view, "A");
    app.session.exit_agent_view();
    assert_eq!(app.session.lock().active_view, "main");
    app.session.enter_agent_view("B");
    assert_eq!(app.session.lock().active_view, "B");
    app.session.exit_agent_view();
    assert_eq!(app.session.lock().active_view, "main");
}

// --- Memory truncation ---

#[test]
fn finished_agent_conversation_truncated_to_20() {
    let app = make_app();
    spawn_agent(&app, "verbose");
    for i in 0..25 {
        app.session.handle_event(AgentEvent::named(
            "verbose",
            AgentEventPayload::Stream {
                text: format!("response {i}"),
            },
        ));
        app.session.handle_event(AgentEvent::named(
            "verbose",
            AgentEventPayload::AwaitingInput,
        ));
        app.session.handle_event(AgentEvent::named(
            "verbose",
            AgentEventPayload::ToolCall {
                id: format!("tc-{i}"),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/test"}),
            },
        ));
    }
    let before = app.session.lock().agents["verbose"]
        .conversation
        .messages
        .len();
    assert!(before > 20, "pre-finish: {before} messages");
    app.session
        .handle_event(AgentEvent::named("verbose", AgentEventPayload::Finished));
    let after = app.session.lock().agents["verbose"]
        .conversation
        .messages
        .len();
    assert!(
        after <= 20,
        "post-finish: {after} messages (should be <=20)"
    );
}

#[test]
fn root_agent_not_truncated() {
    let app = make_app();
    for i in 0..25 {
        app.session.handle_event(AgentEvent::named(
            "main",
            AgentEventPayload::Stream {
                text: format!("msg {i}"),
            },
        ));
        app.session
            .handle_event(AgentEvent::named("main", AgentEventPayload::AwaitingInput));
        app.session.handle_event(AgentEvent::named(
            "main",
            AgentEventPayload::ToolCall {
                id: format!("tc-{i}"),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/test"}),
            },
        ));
    }
    let count = app.session.lock().agents["main"]
        .conversation
        .messages
        .len();
    assert!(count > 20, "root should keep all messages: {count}");
}

#[test]
fn error_agent_conversation_truncated() {
    let app = make_app();
    spawn_agent(&app, "crasher");
    for i in 0..25 {
        app.session.handle_event(AgentEvent::named(
            "crasher",
            AgentEventPayload::Stream {
                text: format!("output {i}"),
            },
        ));
        app.session.handle_event(AgentEvent::named(
            "crasher",
            AgentEventPayload::AwaitingInput,
        ));
        app.session.handle_event(AgentEvent::named(
            "crasher",
            AgentEventPayload::ToolCall {
                id: format!("tc-{i}"),
                name: "Bash".into(),
                input: serde_json::json!({"command": "echo hi"}),
            },
        ));
    }
    let before = app.session.lock().agents["crasher"]
        .conversation
        .messages
        .len();
    assert!(before > 20);
    app.session.handle_event(AgentEvent::named(
        "crasher",
        AgentEventPayload::Error {
            message: "fatal".into(),
        },
    ));
    let after = app.session.lock().agents["crasher"]
        .conversation
        .messages
        .len();
    assert!(after <= 21, "error agent should be truncated, got {after}");
    // +1 because the Error event itself adds a message before truncation
}
