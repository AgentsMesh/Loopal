use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{PermissionIntent, PermissionIntentRequest};
use tokio::sync::mpsc;

use super::InteractionKind;
use super::interaction::PendingInteraction;
use super::store;
use crate::Hub;
use crate::pending_relay::types::{
    InteractionAudience, PendingPermissionInfo, PendingPlanApprovalInfo, PendingQuestionInfo,
};

fn connection() -> Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

fn hub() -> Hub {
    let (events, _rx) = mpsc::channel(8);
    Hub::new(events)
}

fn permission(connection: Arc<Connection<loopal_ipc::Listening>>) -> PendingPermissionInfo {
    let request = PermissionIntentRequest::create(
        "permission",
        "Bash",
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap();
    PendingPermissionInfo {
        execution: crate::types::AgentExecutionRef::local("main", 1),
        agent_conn: connection,
        agent_ipc_id: 5,
        agent_name: "main".into(),
        interaction_id: "permission-token".into(),
        logical_id: "permission".into(),
        tool_name: "Bash".into(),
        permission_intent: PermissionIntent::bind(request.intent_seed, 1, 1, "permission-token")
            .unwrap(),
    }
}

fn question(
    connection: Arc<Connection<loopal_ipc::Listening>>,
    ipc_id: i64,
    logical_id: &str,
) -> PendingQuestionInfo {
    PendingQuestionInfo {
        agent_conn: connection,
        agent_ipc_id: ipc_id,
        agent_name: "main".into(),
        interaction_id: format!("{logical_id}-token"),
        logical_id: logical_id.into(),
        audience: InteractionAudience::LocalUi,
    }
}

fn plan(
    connection: Arc<Connection<loopal_ipc::Listening>>,
    logical_id: &str,
) -> PendingPlanApprovalInfo {
    PendingPlanApprovalInfo {
        agent_conn: connection,
        agent_ipc_id: 20,
        agent_name: "main".into(),
        interaction_id: format!("{logical_id}-token"),
        logical_id: logical_id.into(),
    }
}

fn insert_all(hub: &mut Hub, connection: Arc<Connection<loopal_ipc::Listening>>) {
    hub.pending_permissions.insert(
        ("main".into(), "permission".into()),
        permission(connection.clone()),
    );
    hub.pending_questions.insert(
        ("main".into(), "question".into()),
        question(connection.clone(), 10, "question"),
    );
    hub.pending_plan_approvals
        .insert(("main".into(), "plan".into()), plan(connection, "plan"));
}

#[tokio::test]
async fn request_cancellation_requires_exact_connection() {
    let stale = connection();
    let current = connection();
    let mut hub = hub();
    insert_all(&mut hub, current.clone());

    assert!(store::take_by_request(&mut hub, "missing", &current, 5).is_none());
    for id in [5, 10, 20] {
        assert!(store::take_by_request(&mut hub, "main", &stale, id).is_none());
    }
    assert!(matches!(
        store::take_by_request(&mut hub, "main", &current, 5),
        Some(PendingInteraction::Permission { .. })
    ));
    assert!(matches!(
        store::take_by_request(&mut hub, "main", &current, 10),
        Some(PendingInteraction::Question { .. })
    ));
    assert!(matches!(
        store::take_by_request(&mut hub, "main", &current, 20),
        Some(PendingInteraction::PlanApproval { .. })
    ));
}

#[tokio::test]
async fn connection_cleanup_only_takes_owned_records() {
    let stale = connection();
    let current = connection();
    let mut hub = hub();
    insert_all(&mut hub, current);
    hub.pending_questions.insert(
        ("main".into(), "stale-question".into()),
        question(stale.clone(), 30, "stale-question"),
    );
    hub.pending_plan_approvals.insert(
        ("main".into(), "stale-plan".into()),
        plan(stale.clone(), "stale-plan"),
    );

    assert!(store::take_for_agent_connection(&mut hub, "missing", &stale).is_empty());
    assert_eq!(
        store::take_for_agent_connection(&mut hub, "main", &stale).len(),
        2
    );
    assert_eq!(store::take_for_agent(&mut hub, "main").len(), 3);
}

#[tokio::test]
async fn generation_tokens_guard_every_interaction_kind() {
    let current = connection();
    let mut hub = hub();
    insert_all(&mut hub, current);

    for (kind, logical_id) in [
        (InteractionKind::Permission, "permission"),
        (InteractionKind::Question, "question"),
        (InteractionKind::PlanApproval, "plan"),
    ] {
        assert!(
            store::take_if_generation(&mut hub, kind, "main", logical_id, "stale-token").is_none()
        );
        assert!(
            store::take_if_generation(
                &mut hub,
                kind,
                "main",
                logical_id,
                &format!("{logical_id}-token"),
            )
            .is_some()
        );
    }
}

#[tokio::test]
async fn unavailable_selectors_respect_kind_and_remote_audience() {
    let current = connection();
    let mut hub = hub();
    insert_all(&mut hub, current.clone());
    let mut remote = question(current, 30, "remote");
    remote.audience = InteractionAudience::RemoteUi {
        target_hub: "other".into(),
        uplink: Arc::new(crate::HubUplink::new(connection(), "hub".into())),
    };
    hub.pending_questions
        .insert(("main".into(), "remote".into()), remote);

    assert!(store::take_unavailable(&mut hub, false, false, false).is_empty());
    assert_eq!(store::take_unavailable(&mut hub, true, true, true).len(), 3);
    assert!(
        hub.pending_questions
            .contains_key(&("main".into(), "remote".into()))
    );
}
