use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, McpServerSnapshot, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, McpAction, McpPageState, SubPage};

use tokio::sync::mpsc;

fn make_app_with_rx() -> (App, mpsc::Receiver<ControlCommand>) {
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "m".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    (App::new(session, std::env::temp_dir()), control_rx)
}

fn make_app() -> App {
    make_app_with_rx().0
}

fn servers() -> Vec<McpServerSnapshot> {
    vec![
        McpServerSnapshot {
            name: "a".into(),
            transport: "stdio".into(),
            source: "project".into(),
            status: "connected".into(),
            tool_count: 2,
            resource_count: 0,
            prompt_count: 0,
            errors: vec![],
        },
        McpServerSnapshot {
            name: "b".into(),
            transport: "streamable-http".into(),
            source: "global".into(),
            status: "failed: err".into(),
            tool_count: 0,
            resource_count: 0,
            prompt_count: 0,
            errors: vec!["err".into()],
        },
    ]
}

// --- McpPageState ---

#[test]
fn test_state_new_none_not_loaded() {
    let s = McpPageState::new(None);
    assert!(!s.loaded);
    assert!(s.servers.is_empty());
}

#[test]
fn test_state_new_empty_loaded() {
    let s = McpPageState::new(Some(vec![]));
    assert!(s.loaded);
    assert!(s.servers.is_empty());
}

#[test]
fn test_state_new_with_servers() {
    let s = McpPageState::new(Some(servers()));
    assert!(s.loaded);
    assert_eq!(s.servers.len(), 2);
    assert_eq!(s.selected, 0);
}

#[test]
fn test_selected_server() {
    let mut s = McpPageState::new(Some(servers()));
    assert_eq!(s.selected_server().unwrap().name, "a");
    s.selected = 1;
    assert_eq!(s.selected_server().unwrap().name, "b");
}

#[test]
fn test_selected_out_of_bounds() {
    let mut s = McpPageState::new(Some(servers()));
    s.selected = 99;
    assert!(s.selected_server().is_none());
}

// --- Event → SessionState caching ---

#[test]
fn test_mcp_status_caching() {
    let app = make_app();
    assert!(app.session.lock().mcp_status.is_none());
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: servers(),
        }));
    assert_eq!(app.session.lock().mcp_status.as_ref().unwrap().len(), 2);
    // Update replaces previous
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: vec![servers()[0].clone()],
        }));
    assert_eq!(app.session.lock().mcp_status.as_ref().unwrap().len(), 1);
}

#[test]
fn test_empty_report_sets_some() {
    let app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: vec![],
        }));
    assert!(app.session.lock().mcp_status.as_ref().unwrap().is_empty());
}

// --- Command registry ---

#[test]
fn test_mcp_command_registered() {
    let app = make_app();
    let h = app.command_registry.find("/mcp").unwrap();
    assert!(!h.is_skill());
}

// --- Control dispatch ---

#[tokio::test]
async fn test_reconnect_sends_control() {
    let (mut app, mut rx) = make_app_with_rx();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    let target = app.session.lock().active_view.clone();
    app.session
        .send_control(target, ControlCommand::McpReconnect { server: "s".into() })
        .await;
    let cmd = rx.try_recv().unwrap();
    assert!(matches!(
        cmd,
        ControlCommand::McpReconnect { server } if server == "s"
    ));
}

#[tokio::test]
async fn test_disconnect_sends_control() {
    let (app, mut rx) = make_app_with_rx();
    let target = app.session.lock().active_view.clone();
    app.session
        .send_control(target, ControlCommand::McpDisconnect { server: "x".into() })
        .await;
    let cmd = rx.try_recv().unwrap();
    assert!(matches!(
        cmd,
        ControlCommand::McpDisconnect { server } if server == "x"
    ));
}

// --- ActionMenu ---

#[test]
fn test_action_menu_connected_vs_failed() {
    let mut s = McpPageState::new(Some(servers()));
    assert!(s.action_menu.is_none());

    // connected server → Disconnect + Reconnect
    s.selected = 0;
    s.open_action_menu();
    let menu = s.action_menu.as_ref().unwrap();
    assert_eq!(menu.server_name, "a");
    assert_eq!(
        menu.options,
        vec![McpAction::Disconnect, McpAction::Reconnect]
    );

    // failed server → Reconnect only
    s.selected = 1;
    s.open_action_menu();
    let menu = s.action_menu.as_ref().unwrap();
    assert_eq!(menu.server_name, "b");
    assert_eq!(menu.options, vec![McpAction::Reconnect]);
}

#[test]
fn test_open_menu_empty_list_noop() {
    let mut s = McpPageState::new(Some(vec![]));
    s.open_action_menu();
    assert!(s.action_menu.is_none());
}
