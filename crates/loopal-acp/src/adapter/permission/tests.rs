use std::sync::Arc;

use loopal_agent_hub::{HubClient, UiSession};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::PermissionIntentDigest;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

use super::*;

async fn permission_round_trip(
    outcome: Option<Value>,
    digest: Option<PermissionIntentDigest>,
) -> (Value, Value) {
    let (hub_transport, client_transport) = loopal_ipc::duplex_pair();
    let (hub_connection, mut hub_rx) = Connection::new(hub_transport).into_listening();
    let (client_connection, mut client_rx) = Connection::new(client_transport).into_listening();
    tokio::spawn(async move { while client_rx.recv().await.is_some() {} });
    let (_event_tx, event_rx) = tokio::sync::broadcast::channel(8);
    let ui = UiSession {
        client: Arc::new(HubClient::new(client_connection)),
        event_rx,
        lease_id: "test-ui".into(),
    };
    let (acp_writer, acp_reader) = tokio::io::duplex(4096);
    let adapter = Arc::new(AcpAdapter::new(
        ui,
        Arc::new(crate::jsonrpc::JsonRpcTransport::with_writer(Box::new(
            acp_writer,
        ))),
    ));
    let task = tokio::spawn({
        let adapter = adapter.clone();
        async move {
            adapter
                .handle_permission_request(
                    "worker".into(),
                    "permission-1".into(),
                    "Bash".into(),
                    json!({"command": "pwd"}),
                    digest,
                    "session-1",
                )
                .await;
        }
    });

    let mut reader = BufReader::new(acp_reader);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let request: Value = serde_json::from_str(line.trim()).unwrap();
    let request_id = request["id"].as_i64().unwrap();
    if let Some(outcome) = outcome {
        adapter.acp_out.route_response(request_id, outcome).await;
    } else {
        adapter.acp_out.drop_pending_response(request_id).await;
    }

    let Incoming::Request { id, method, params } = hub_rx.recv().await.unwrap() else {
        panic!("expected hub permission response");
    };
    assert_eq!(method, "hub/permission_response");
    hub_connection.respond(id, json!({})).await.unwrap();
    task.await.unwrap();
    (request, params)
}

#[tokio::test]
async fn selected_allow_forwards_shape_and_intent_digest() {
    let digest = PermissionIntentDigest::from_bytes([0x5a; 32]);
    let (request, response) = permission_round_trip(
        Some(json!({"outcome":{"outcome":"selected","optionId":"allow_once"}})),
        Some(digest),
    )
    .await;

    assert_eq!(request["method"], "session/request_permission");
    assert_eq!(request["params"]["sessionId"], "session-1");
    assert_eq!(request["params"]["toolCall"]["toolCallId"], "permission-1");
    assert_eq!(request["params"]["toolCall"]["title"], "Bash");
    assert_eq!(request["params"]["toolCall"]["status"], "pending");
    assert_eq!(
        request["params"]["toolCall"]["rawInput"],
        json!({"command":"pwd"})
    );
    let options = request["params"]["options"].as_array().unwrap();
    assert_eq!(
        options
            .iter()
            .map(|option| (&option["optionId"], &option["kind"]))
            .collect::<Vec<_>>(),
        vec![
            (&json!("allow_once"), &json!("allow_once")),
            (&json!("allow_always"), &json!("allow_always")),
            (&json!("reject_once"), &json!("reject_once")),
            (&json!("reject_always"), &json!("reject_always")),
        ]
    );
    assert_eq!(response["agent_name"], "worker");
    assert_eq!(response["tool_call_id"], "permission-1");
    assert_eq!(response["permission_intent_digest"], json!(digest));
    assert_eq!(response["allow"], true);
    assert_eq!(response["remember_session"], false);
}

#[tokio::test]
async fn allow_always_forwards_session_memory() {
    let (_, response) = permission_round_trip(
        Some(json!({"outcome":{"outcome":"selected","optionId":"allow_always"}})),
        None,
    )
    .await;
    assert_eq!(response["allow"], true);
    assert_eq!(response["remember_session"], true);
}

#[tokio::test]
async fn cancelled_denies_without_intent_digest() {
    let (_, response) =
        permission_round_trip(Some(json!({"outcome":{"outcome":"cancelled"}})), None).await;
    assert!(response["permission_intent_digest"].is_null());
    assert_eq!(response["allow"], false);
}

#[tokio::test]
async fn failed_ide_request_denies() {
    let (_, response) = permission_round_trip(None, None).await;
    assert_eq!(response["allow"], false);
}

#[test]
fn parses_current_legacy_and_malformed_outcomes() {
    assert_eq!(
        parse_permission_outcome(
            &json!({"outcome":{"outcome":"selected","optionId":"allow_once"}})
        ),
        PermissionSelection {
            allow: true,
            remember_session: false,
        }
    );
    assert_eq!(
        parse_permission_outcome(
            &json!({"outcome":{"outcome":"selected","optionId":"allow_always"}})
        ),
        PermissionSelection {
            allow: true,
            remember_session: true,
        }
    );
    for value in [
        json!({"outcome":{"outcome":"selected","optionId":"reject_once"}}),
        json!({"outcome":{"outcome":"selected","optionId":"reject_always"}}),
        json!({"outcome":{"outcome":"cancelled"}}),
        json!({"outcome":"deny"}),
        json!({}),
        json!(null),
        json!(42),
    ] {
        assert_eq!(parse_permission_outcome(&value), PermissionSelection::DENY);
    }
    assert_eq!(
        parse_permission_outcome(&json!({"outcome":"allow"})),
        PermissionSelection {
            allow: true,
            remember_session: false,
        }
    );
}
