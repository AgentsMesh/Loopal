use loopal_protocol::{AgentEvent, AgentEventPayload, McpServerSnapshot, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, McpPageState, SubPage};

fn make_app() -> App {
    let (control_tx, _) = tokio::sync::mpsc::channel(16);
    let (perm_tx, _) = tokio::sync::mpsc::channel::<bool>(16);
    let (question_tx, _) = tokio::sync::mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "m".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
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

/// Simulate what tui_loop::refresh_mcp_page does.
fn refresh(app: &mut App) {
    if let Some(SubPage::McpPage(ref mut state)) = app.sub_page {
        let servers = app.session.lock().mcp_status.clone().unwrap_or_default();
        state.selected = state.selected.min(servers.len().saturating_sub(1));
        state.scroll_offset = state.scroll_offset.min(servers.len().saturating_sub(1));
        state.servers = servers;
        state.loaded = true;
    }
}

#[test]
fn test_refresh_populates_unloaded_page() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(None)));
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: servers(),
        }));
    refresh(&mut app);
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("expected McpPage"),
    };
    assert!(s.loaded);
    assert_eq!(s.servers.len(), 2);
}

#[test]
fn test_refresh_clamps_selection() {
    let mut app = make_app();
    let mut state = McpPageState::new(Some(servers()));
    state.selected = 1;
    state.scroll_offset = 1;
    app.sub_page = Some(SubPage::McpPage(state));

    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: vec![servers()[0].clone()],
        }));
    refresh(&mut app);

    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("expected McpPage"),
    };
    assert_eq!(s.selected, 0);
    assert_eq!(s.scroll_offset, 0);
    assert_eq!(s.servers.len(), 1);
}

#[test]
fn test_refresh_noop_when_page_closed() {
    let mut app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: servers(),
        }));
    refresh(&mut app);
    assert!(app.sub_page.is_none());
    assert!(app.session.lock().mcp_status.is_some());
}

#[test]
fn test_refresh_to_empty_list() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(Some(servers()))));
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
            servers: vec![],
        }));
    refresh(&mut app);
    let s = match app.sub_page.as_ref().unwrap() {
        SubPage::McpPage(s) => s,
        _ => panic!("expected McpPage"),
    };
    assert!(s.loaded);
    assert!(s.servers.is_empty());
    assert_eq!(s.selected, 0);
}
