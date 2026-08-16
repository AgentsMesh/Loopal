//! Unit tests for `translate::translate_event` — the dispatch table that
//! converts `AgentEventPayload` to `AcpNotification`. Each variant must
//! land in exactly one of: SessionUpdate, Extension, or None.

use loopal_acp::translate::{AcpNotification, translate_event};
use loopal_protocol::{
    AgentEventPayload, BgTaskStatus, GoalTransitionReason, WorkflowNodeId, WorkflowRunId,
    WorkflowRunState, WorkflowRunSummary, WorkflowStateCounts,
};

#[test]
fn stream_returns_session_update() {
    let r = translate_event(&AgentEventPayload::Stream { text: "hi".into() }, "s");
    assert!(matches!(r, Some(AcpNotification::SessionUpdate(_))));
}

#[test]
fn thinking_returns_session_update() {
    let r = translate_event(&AgentEventPayload::ThinkingStream { text: "t".into() }, "s");
    assert!(matches!(r, Some(AcpNotification::SessionUpdate(_))));
}

#[test]
fn retry_error_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::RetryError {
            message: "e".into(),
            attempt: 1,
            max_attempts: 3,
        },
        "s",
    );
    assert!(matches!(r, Some(AcpNotification::Extension { .. })));
}

#[test]
fn session_resume_warnings_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::SessionResumeWarnings {
            session_id: "s1".into(),
            warnings: vec!["cron load failed".into()],
        },
        "s1",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/sessionResumeWarnings");
            assert_eq!(params["sessionId"], "s1");
            assert_eq!(params["data"]["warnings"][0], "cron load failed");
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn session_resumed_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::SessionResumed {
            session_id: "s2".into(),
            message_count: 7,
        },
        "s2",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/sessionResumed");
            assert_eq!(params["sessionId"], "s2");
            assert_eq!(params["data"]["messageCount"], 7);
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn none_events_return_none() {
    let nones = vec![
        AgentEventPayload::AwaitingInput,
        AgentEventPayload::Started,
        AgentEventPayload::Running,
        AgentEventPayload::Finished,
        AgentEventPayload::Interrupted,
        AgentEventPayload::RetryCleared,
        AgentEventPayload::HubDegraded { since_unix_ms: 1 },
        AgentEventPayload::HubRecovered { duration_ms: 2 },
    ];
    for ev in &nones {
        assert!(
            translate_event(ev, "s").is_none(),
            "expected None for {ev:?}"
        );
    }
}

#[test]
fn inbox_enqueued_human_returns_none_to_avoid_echoing_input_back() {
    let r = translate_event(
        &AgentEventPayload::InboxEnqueued {
            envelope_id: "m".into(),
            source: loopal_protocol::MessageSource::Human,
            content: "hi".into(),
            summary: None,
        },
        "s",
    );
    assert!(r.is_none());
}

#[test]
fn inbox_enqueued_agent_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::InboxEnqueued {
            envelope_id: "m".into(),
            source: loopal_protocol::MessageSource::Agent(
                loopal_protocol::QualifiedAddress::local("worker"),
            ),
            content: "ping".into(),
            summary: Some("hello".into()),
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/inbox.enqueued");
            assert_eq!(params["data"]["summary"], "hello");
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn inbox_consumed_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::InboxConsumed {
            envelope_id: "m-7".into(),
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/inbox.consumed");
            assert_eq!(params["data"]["messageId"], "m-7");
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn cleared_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::Cleared {
            context_window: 200_000,
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/cleared");
            assert_eq!(params["data"]["contextWindow"], 200_000);
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn model_changed_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::ModelChanged {
            model: "claude-opus-4-7".into(),
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/model");
            assert_eq!(params["data"]["model"], "claude-opus-4-7");
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn thinking_changed_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::ThinkingChanged {
            thinking_config: r#"{"type":"effort","level":"high"}"#.into(),
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/thinking");
            assert_eq!(
                params["data"]["thinking"],
                r#"{"type":"effort","level":"high"}"#
            );
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn bg_task_spawned_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::BgTaskSpawned {
            id: "bg1".into(),
            description: "npm test".into(),
            created_at_unix_ms: 1_717_000_000_000,
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/bgTask.spawned");
            assert_eq!(params["data"]["id"], "bg1");
            assert_eq!(params["data"]["description"], "npm test");
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn bg_task_output_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::BgTaskOutput {
            id: "bg1".into(),
            output_delta: "line\n".into(),
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/bgTask.output");
            assert_eq!(params["data"]["output_delta"], "line\n");
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn bg_task_completed_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::BgTaskCompleted {
            id: "bg1".into(),
            status: loopal_protocol::BgTaskStatus::Completed,
            exit_code: Some(0),
            output: "done".into(),
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/bgTask.completed");
            assert_eq!(params["data"]["exit_code"], 0);
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn crons_changed_returns_extension() {
    match translate_event(&AgentEventPayload::CronsChanged { crons: vec![] }, "s") {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/crons");
            assert!(params["data"]["crons"].is_array());
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn tasks_changed_returns_extension() {
    match translate_event(&AgentEventPayload::TasksChanged { tasks: vec![] }, "s") {
        Some(AcpNotification::Extension { method, .. }) => assert_eq!(method, "_loopal/tasks"),
        _ => panic!("expected Extension"),
    }
}

#[test]
fn mcp_status_report_returns_extension() {
    match translate_event(&AgentEventPayload::McpStatusReport { servers: vec![] }, "s") {
        Some(AcpNotification::Extension { method, .. }) => assert_eq!(method, "_loopal/mcp"),
        _ => panic!("expected Extension"),
    }
}

#[test]
fn sub_agent_spawned_returns_topology_extension() {
    let r = translate_event(
        &AgentEventPayload::SubAgentSpawned(loopal_protocol::SubAgentSpawn {
            name: "worker".into(),
            agent_id: "a1".into(),
            parent: None,
            model: None,
            session_id: None,
        }),
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/topology.spawn");
            assert_eq!(params["data"]["spawn"]["name"], "worker");
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn thread_goal_updated_returns_goal_extension() {
    let r = translate_event(
        &AgentEventPayload::ThreadGoalUpdated {
            goal: None,
            reason: loopal_protocol::GoalTransitionReason::UserCleared,
        },
        "s",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/goal");
            assert!(params["data"].get("goal").is_some());
        }
        _ => panic!("expected Extension"),
    }
}

#[test]
fn workflow_run_changed_returns_typed_workflow_extension() {
    let workflow = WorkflowRunSummary {
        id: WorkflowRunId::new("wrun-1"),
        run_goal: "ship it".into(),
        state: WorkflowRunState::Running,
        revision: 3,
        output_node: WorkflowNodeId::new("output"),
        counts: WorkflowStateCounts {
            pending: 1,
            ready: 2,
            active: 3,
            succeeded: 4,
            failed: 5,
            cancelled: 6,
            skipped: 7,
        },
        created_at_unix_ms: 10,
        updated_at_unix_ms: 20,
    };

    match translate_event(
        &AgentEventPayload::WorkflowRunChanged(workflow.clone()),
        "s",
    ) {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/workflow");
            assert_eq!(params["sessionId"], "s");
            assert_eq!(
                params["data"]["workflow"],
                serde_json::to_value(workflow).unwrap()
            );
        }
        _ => panic!("expected Extension"),
    }
}

// The inputs whose translate_event output each golden fixture entry must be a
// superset of. Complex snapshot types are built via from_value so the test
// doesn't hand-write (and drift on) their full field set.
fn payload_for_kind(kind: &str) -> AgentEventPayload {
    use AgentEventPayload as P;
    use loopal_protocol::{
        CronJobSnapshot, McpServerSnapshot, SubAgentSpawn, TaskSnapshot, ThreadGoal,
    };
    use serde_json::{from_value, json};
    match kind {
        "bgTask.spawned" => P::BgTaskSpawned {
            id: "bg1".into(),
            description: "npm test".into(),
            created_at_unix_ms: 1_717_000_000_000,
        },
        "bgTask.output" => P::BgTaskOutput {
            id: "bg1".into(),
            output_delta: "running...\n".into(),
        },
        "bgTask.completed" => P::BgTaskCompleted {
            id: "bg1".into(),
            status: BgTaskStatus::Completed,
            exit_code: Some(0),
            output: "done".into(),
        },
        "crons" => P::CronsChanged {
            crons: from_value::<Vec<CronJobSnapshot>>(json!([{
                "id": "c1", "cron_expr": "0 9 * * *", "prompt": "daily",
                "recurring": true, "created_at_unix_ms": 0, "next_fire_unix_ms": null, "durable": true
            }]))
            .unwrap(),
        },
        "tasks" => P::TasksChanged {
            tasks: from_value::<Vec<TaskSnapshot>>(json!([{
                "id": "t1", "subject": "build", "active_form": null,
                "status": "in_progress", "blocked_by": [], "description": "", "blocks": []
            }]))
            .unwrap(),
        },
        "topology.spawn" => P::SubAgentSpawned(SubAgentSpawn {
            name: "worker".into(),
            agent_id: "a1".into(),
            parent: Some(loopal_protocol::QualifiedAddress::local("main")),
            model: Some("opus".into()),
            session_id: None,
        }),
        "mcp" => P::McpStatusReport {
            servers: from_value::<Vec<McpServerSnapshot>>(json!([{
                "name": "fs", "transport": "stdio", "source": "global", "status": "connected",
                "tool_count": 3, "resource_count": 0, "prompt_count": 0, "errors": []
            }]))
            .unwrap(),
        },
        "goal" => P::ThreadGoalUpdated {
            goal: Some(
                from_value::<ThreadGoal>(json!({
                    "session_id": "s", "goal_id": "g1", "objective": "ship it", "status": "active",
                    "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z"
                }))
                .unwrap(),
            ),
            reason: GoalTransitionReason::UserCleared,
        },
        _ => panic!("unknown kind: {kind}"),
    }
}

// True when `actual` contains every field of `expected` (recursive subset).
// Loopal emits the full wire; the fixture is the field subset AgentsMesh
// consumes, so a superset is correct — extra Loopal fields don't fail.
fn json_contains(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e
            .iter()
            .all(|(k, ev)| a.get(k).is_some_and(|av| json_contains(av, ev))),
        (Value::Array(a), Value::Array(e)) => {
            a.len() == e.len() && a.iter().zip(e).all(|(av, ev)| json_contains(av, ev))
        }
        _ => actual == expected,
    }
}

#[test]
fn panel_signals_match_golden_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/loopal_panel_signals.json")).unwrap();
    for entry in fixture["signals"].as_array().unwrap() {
        let kind = entry["kind"].as_str().unwrap();
        let want = &entry["data"];
        match translate_event(&payload_for_kind(kind), "s") {
            Some(AcpNotification::Extension { method, params }) => {
                assert_eq!(method, format!("_loopal/{kind}"));
                assert!(
                    json_contains(&params["data"], want),
                    "fixture drift for {kind}:\n  produced={}\n  fixture={want}",
                    params["data"]
                );
            }
            _ => panic!("expected Extension for {kind}"),
        }
    }
}
