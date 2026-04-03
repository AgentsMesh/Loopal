/// Integration tests for agent view switching: Tab/Enter/ESC/Up/Down/Delete/Ctrl+C interactions.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{
    AgentEvent, AgentEventPayload, AgentStatus, ControlCommand, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::{InputAction, handle_key};

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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Spawn a sub-agent by injecting SubAgentSpawned + Started events.
fn spawn_agent(app: &App, name: &str) {
    app.session.handle_event(AgentEvent::named(
        name,
        AgentEventPayload::SubAgentSpawned {
            name: name.to_string(),
            agent_id: "id-1".into(),
            parent: Some("main".into()),
            model: Some("test-model".into()),
            session_id: None,
        },
    ));
    app.session
        .handle_event(AgentEvent::named(name, AgentEventPayload::Started));
}

// === Tab focuses sub-agents ===

#[test]
fn tab_focuses_first_live_subagent() {
    let mut app = make_app();
    spawn_agent(&app, "researcher");
    let action = handle_key(&mut app, key(KeyCode::Tab));
    assert!(matches!(action, InputAction::EnterPanel));
    // Simulate dispatch
    loopal_tui::input::handle_key(&mut app, key(KeyCode::Tab));
    // Tab returns EnterPanel, which key_dispatch handles
}

#[test]
fn tab_skips_main_agent() {
    let app = make_app();
    spawn_agent(&app, "worker");
    // After Tab, focused_agent should be "worker", not "main"
    // We simulate the dispatch manually since handle_key just returns the action
    let state = app.session.lock();
    let active = state.active_view.clone();
    let keys: Vec<String> = state
        .agents
        .iter()
        .filter(|(k, a)| {
            k.as_str() != active
                && !matches!(
                    a.observable.status,
                    AgentStatus::Finished | AgentStatus::Error
                )
        })
        .map(|(k, _)| k.clone())
        .collect();
    drop(state);
    assert_eq!(keys, vec!["worker"]);
    assert!(!keys.contains(&"main".to_string()));
}

#[test]
fn tab_skips_finished_agents() {
    let app = make_app();
    spawn_agent(&app, "alive");
    spawn_agent(&app, "dead");
    // Finish "dead"
    app.session
        .handle_event(AgentEvent::named("dead", AgentEventPayload::Finished));
    let state = app.session.lock();
    let active = state.active_view.clone();
    let live_keys: Vec<String> = state
        .agents
        .iter()
        .filter(|(k, a)| {
            k.as_str() != active
                && !matches!(
                    a.observable.status,
                    AgentStatus::Finished | AgentStatus::Error
                )
        })
        .map(|(k, _)| k.clone())
        .collect();
    drop(state);
    assert_eq!(live_keys, vec!["alive"]);
}

// === Enter drills into focused agent ===

#[test]
fn enter_on_empty_input_with_focus_returns_enter_agent_view() {
    let mut app = make_app();
    spawn_agent(&app, "researcher");
    app.focused_agent = Some("researcher".into());
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::EnterAgentView));
}

#[test]
fn enter_with_text_does_not_drill_in() {
    let mut app = make_app();
    spawn_agent(&app, "researcher");
    app.focused_agent = Some("researcher".into());
    app.input = "hello".into();
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::InboxPush(_)));
}

#[test]
fn enter_when_focused_equals_active_view_does_not_drill() {
    let mut app = make_app();
    // focused on "main" which is already active_view → no drill
    app.focused_agent = Some("main".into());
    let action = handle_key(&mut app, key(KeyCode::Enter));
    // Should be None (empty input, focused == active)
    assert!(matches!(action, InputAction::None));
}

#[test]
fn enter_agent_view_blocks_finished_agent() {
    let app = make_app();
    spawn_agent(&app, "done-agent");
    app.session
        .handle_event(AgentEvent::named("done-agent", AgentEventPayload::Finished));
    let result = app.session.enter_agent_view("done-agent");
    assert!(!result, "should not enter a finished agent");
    assert_eq!(app.session.lock().active_view, "main");
}

// === ESC exits agent view ===

#[test]
fn esc_exits_agent_view_back_to_root() {
    let mut app = make_app();
    spawn_agent(&app, "worker");
    app.session.enter_agent_view("worker");
    assert_eq!(app.session.lock().active_view, "worker");
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(matches!(action, InputAction::ExitAgentView));
}

#[test]
fn esc_when_viewing_main_does_not_exit_view() {
    let mut app = make_app();
    // agent is idle, so ESC → double-ESC rewind path (not ExitAgentView)
    app.session
        .handle_event(AgentEvent::named("main", AgentEventPayload::AwaitingInput));
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(!matches!(action, InputAction::ExitAgentView));
}

// === Auto-return on Finished/Error ===

#[test]
fn auto_return_to_root_on_viewed_agent_finished() {
    let app = make_app();
    spawn_agent(&app, "worker");
    app.session.enter_agent_view("worker");
    assert_eq!(app.session.lock().active_view, "worker");
    app.session
        .handle_event(AgentEvent::named("worker", AgentEventPayload::Finished));
    assert_eq!(app.session.lock().active_view, "main");
}

#[test]
fn auto_return_to_root_on_viewed_agent_error() {
    let app = make_app();
    spawn_agent(&app, "worker");
    app.session.enter_agent_view("worker");
    app.session.handle_event(AgentEvent::named(
        "worker",
        AgentEventPayload::Error {
            message: "boom".into(),
        },
    ));
    assert_eq!(app.session.lock().active_view, "main");
}

#[test]
fn no_auto_return_when_not_viewing_finished_agent() {
    let app = make_app();
    spawn_agent(&app, "worker");
    // Still viewing "main", worker finishes → active_view stays "main"
    app.session
        .handle_event(AgentEvent::named("worker", AgentEventPayload::Finished));
    assert_eq!(app.session.lock().active_view, "main");
}

// === Sub-agent events write to correct conversation ===

#[test]
fn sub_agent_stream_writes_to_agent_conversation() {
    let app = make_app();
    spawn_agent(&app, "researcher");
    app.session.handle_event(AgentEvent::named(
        "researcher",
        AgentEventPayload::Stream {
            text: "finding...".into(),
        },
    ));
    let state = app.session.lock();
    assert_eq!(
        state.agents["researcher"].conversation.streaming_text,
        "finding..."
    );
    // Root conversation should be empty
    assert!(state.agents["main"].conversation.streaming_text.is_empty());
}

#[test]
fn viewed_agent_conversation_reflects_active_view() {
    let app = make_app();
    spawn_agent(&app, "coder");
    app.session.handle_event(AgentEvent::named(
        "coder",
        AgentEventPayload::Stream {
            text: "code".into(),
        },
    ));
    // active_conversation shows main (default view)
    assert!(
        app.session
            .lock()
            .active_conversation()
            .streaming_text
            .is_empty()
    );
    // Switch view
    app.session.enter_agent_view("coder");
    assert_eq!(
        app.session.lock().active_conversation().streaming_text,
        "code"
    );
}

// === ESC priority: exit view > interrupt ===

#[test]
fn esc_exits_view_even_when_agent_busy() {
    let mut app = make_app();
    spawn_agent(&app, "busy");
    // Agent is busy (not idle — just started, never got AwaitingInput)
    app.session.enter_agent_view("busy");
    let action = handle_key(&mut app, key(KeyCode::Esc));
    // Should exit view, NOT interrupt
    assert!(matches!(action, InputAction::ExitAgentView));
}

// === Enter blocks Error agents too ===

#[test]
fn enter_agent_view_blocks_error_agent() {
    let app = make_app();
    spawn_agent(&app, "broken");
    app.session.handle_event(AgentEvent::named(
        "broken",
        AgentEventPayload::Error {
            message: "crash".into(),
        },
    ));
    let result = app.session.enter_agent_view("broken");
    assert!(!result, "should not enter an errored agent");
}

// === ToolCall events route to correct conversation ===

#[test]
fn sub_agent_tool_call_writes_to_agent_conversation() {
    let app = make_app();
    spawn_agent(&app, "worker");
    app.session.handle_event(AgentEvent::named(
        "worker",
        AgentEventPayload::ToolCall {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/x"}),
        },
    ));
    let state = app.session.lock();
    let worker_conv = &state.agents["worker"].conversation;
    assert!(
        !worker_conv.messages.is_empty(),
        "worker should have tool call message"
    );
    let main_conv = &state.agents["main"].conversation;
    assert!(
        main_conv.messages.is_empty(),
        "main should have no messages"
    );
}

// === Tab cycles through multiple agents ===

#[test]
fn tab_cycles_through_multiple_agents_in_order() {
    let app = make_app();
    spawn_agent(&app, "a");
    spawn_agent(&app, "b");
    spawn_agent(&app, "c");
    // Simulate cycle_agent_focus by reading agent keys
    let state = app.session.lock();
    let keys: Vec<String> = state
        .agents
        .iter()
        .filter(|(k, a)| {
            k.as_str() != "main"
                && !matches!(
                    a.observable.status,
                    AgentStatus::Finished | AgentStatus::Error
                )
        })
        .map(|(k, _)| k.clone())
        .collect();
    drop(state);
    assert_eq!(
        keys,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}
