use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, PermissionIntentDigest, UiCapabilities, UserQuestionResponse};
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};

use super::relay_response_handlers::{
    handle_permission_response, handle_plan_approval_response, handle_question_response,
};
use crate::Hub;
use crate::request_principal::UiPrincipal;

fn hub() -> Arc<Mutex<Hub>> {
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(8);
    Arc::new(Mutex::new(Hub::new(events)))
}

fn ui() -> UiPrincipal {
    let (peer, transport) = loopal_ipc::duplex_pair();
    let (_peer, _peer_rx) = Connection::new(peer).into_listening();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    UiPrincipal::new(
        "lease".into(),
        "desktop".into(),
        UiCapabilities::ALL,
        connection,
    )
}

fn assert_error(result: Result<Value, String>, expected: &str) {
    assert!(result.unwrap_err().contains(expected));
}

#[tokio::test]
async fn permission_response_validates_required_fields_and_digest_defaults() {
    let hub = hub();
    let ui = ui();
    assert_error(
        handle_permission_response(&hub, json!({}), &ui).await,
        "missing agent_name",
    );
    assert_error(
        handle_permission_response(&hub, json!({"agent_name": "worker"}), &ui).await,
        "missing tool_call_id",
    );
    assert_error(
        handle_permission_response(
            &hub,
            json!({"agent_name": "worker", "tool_call_id": "token"}),
            &ui,
        )
        .await,
        "missing allow",
    );

    for params in [
        json!({"agent_name": "worker", "tool_call_id": "token", "allow": false}),
        json!({
            "agent_name": "worker",
            "tool_call_id": "token",
            "allow": true,
            "remember_session": true,
            "permission_intent_digest": PermissionIntentDigest::from_bytes([7; 32]),
        }),
        json!({
            "agent_name": "worker",
            "tool_call_id": "token",
            "allow": true,
            "permission_intent_digest": "invalid",
        }),
    ] {
        assert_eq!(
            handle_permission_response(&hub, params, &ui).await.unwrap(),
            json!({"resolved": false})
        );
    }
}

#[tokio::test]
async fn question_response_validates_body_and_exact_question_id() {
    let hub = hub();
    for (params, expected) in [
        (json!({}), "missing agent_name"),
        (json!({"agent_name": "worker"}), "question_id"),
        (
            json!({"agent_name": "worker", "question_id": ""}),
            "question_id",
        ),
        (
            json!({"agent_name": "worker", "question_id": "token"}),
            "missing response",
        ),
        (
            json!({"agent_name": "worker", "question_id": "token", "response": {}}),
            "bad response",
        ),
    ] {
        assert_error(handle_question_response(&hub, params).await, expected);
    }
    assert_error(
        handle_question_response(
            &hub,
            json!({
                "agent_name": "worker",
                "question_id": "token",
                "response": UserQuestionResponse::cancelled("other"),
            }),
        )
        .await,
        "id mismatch",
    );
    assert_eq!(
        handle_question_response(
            &hub,
            json!({
                "agent_name": "worker",
                "question_id": "token",
                "response": UserQuestionResponse::answered("token", vec!["yes".into()]),
            }),
        )
        .await
        .unwrap(),
        json!({"resolved": false})
    );
}

#[tokio::test]
async fn plan_response_validates_all_decisions_and_optional_edits() {
    let hub = hub();
    for (params, expected) in [
        (json!({}), "agent_name"),
        (json!({"agent_name": "worker"}), "request_id"),
        (
            json!({"agent_name": "worker", "request_id": "token"}),
            "decision",
        ),
        (
            json!({"agent_name": "worker", "request_id": "token", "decision": "later"}),
            "invalid plan approval decision",
        ),
        (
            json!({
                "agent_name": "worker",
                "request_id": "token",
                "decision": "approve_with_edits",
            }),
            "edited_plan",
        ),
    ] {
        assert_error(handle_plan_approval_response(&hub, params).await, expected);
    }
    for params in [
        json!({"agent_name": "worker", "request_id": "token", "decision": "approve"}),
        json!({"agent_name": "worker", "request_id": "token", "decision": "reject"}),
        json!({
            "agent_name": "worker",
            "request_id": "token",
            "decision": "approve_with_edits",
            "edited_plan": "revised",
        }),
    ] {
        assert_eq!(
            handle_plan_approval_response(&hub, params).await.unwrap(),
            json!({"resolved": false})
        );
    }
}
