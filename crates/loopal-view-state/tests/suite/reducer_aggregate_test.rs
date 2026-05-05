//! Aggregate-update events: tasks/crons/mcp full-replacement,
//! sub-agent topology, session resume.

use loopal_protocol::{
    AgentEventPayload, CronJobSnapshot, McpServerSnapshot, TaskSnapshot, TaskSnapshotStatus,
};
use loopal_view_state::ViewStateReducer;

fn task(id: &str, subject: &str) -> TaskSnapshot {
    TaskSnapshot {
        id: id.into(),
        subject: subject.into(),
        active_form: None,
        status: TaskSnapshotStatus::Pending,
        blocked_by: vec![],
    }
}

#[test]
fn tasks_changed_replaces_full_list() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::TasksChanged {
        tasks: vec![task("t1", "first")],
    });
    r.apply(AgentEventPayload::TasksChanged {
        tasks: vec![task("t2", "replaced")],
    });
    assert_eq!(r.state().tasks.len(), 1);
    assert_eq!(r.state().tasks[0].id, "t2");
}

#[test]
fn crons_changed_replaces_full_list() {
    let mut r = ViewStateReducer::new("root");
    let cron = CronJobSnapshot {
        id: "c1".into(),
        cron_expr: "0 * * * *".into(),
        prompt: "tick".into(),
        recurring: true,
        created_at_unix_ms: 0,
        next_fire_unix_ms: None,
        durable: false,
    };
    r.apply(AgentEventPayload::CronsChanged {
        crons: vec![cron.clone()],
    });
    assert_eq!(r.state().crons.len(), 1);
    assert_eq!(r.state().crons[0].id, "c1");
}

#[test]
fn mcp_status_report_replaces_servers() {
    let mut r = ViewStateReducer::new("root");
    let server = McpServerSnapshot {
        name: "fs".into(),
        transport: "stdio".into(),
        source: "global".into(),
        status: "connected".into(),
        tool_count: 3,
        resource_count: 0,
        prompt_count: 0,
        errors: vec![],
    };
    r.apply(AgentEventPayload::McpStatusReport {
        servers: vec![server],
    });
    assert!(r.state().mcp_status.is_some());
    assert_eq!(r.state().mcp_status.as_ref().unwrap().len(), 1);
}

#[test]
fn sub_agent_spawned_appends_child() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::SubAgentSpawned {
        name: "researcher".into(),
        agent_id: "a-001".into(),
        parent: None,
        model: None,
        session_id: None,
    });
    assert_eq!(r.state().agent.children, vec!["researcher".to_string()]);
}

#[test]
fn duplicate_sub_agent_spawn_is_noop() {
    let mut r = ViewStateReducer::new("root");
    let evt = || AgentEventPayload::SubAgentSpawned {
        name: "researcher".into(),
        agent_id: "a-001".into(),
        parent: None,
        model: None,
        session_id: None,
    };
    r.apply(evt());
    let after_first = r.rev();
    let result = r.apply(evt());
    assert!(result.is_none());
    assert_eq!(r.rev(), after_first);
    assert_eq!(r.state().agent.children.len(), 1);
}

#[test]
fn session_resumed_clears_session_state_and_records_id() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::TasksChanged {
        tasks: vec![task("t1", "doomed")],
    });
    r.apply(AgentEventPayload::BgTaskSpawned {
        id: "bg_1".into(),
        description: "doomed bg".into(),
    });
    r.apply(AgentEventPayload::SessionResumed {
        session_id: "session-xyz".into(),
        message_count: 0,
    });
    assert_eq!(r.state().agent.session_id.as_deref(), Some("session-xyz"));
    assert!(r.state().tasks.is_empty());
    assert!(r.state().bg_tasks.is_empty());
    assert!(r.state().crons.is_empty());
}
