use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{
    PermissionActionDigest, PermissionDisplayDigest, PermissionIntent, PermissionIntentSeed,
    PermissionSchemaDigest, QualifiedAddress, Question,
};

use super::*;

fn client_pair() -> (
    Arc<HubClient>,
    Arc<Connection<loopal_ipc::connection::Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client_conn, _client_rx) = Connection::new(client_transport).into_listening();
    let (server_conn, server_rx) = Connection::new(server_transport).into_listening();
    (
        Arc::new(HubClient::new(client_conn)),
        server_conn,
        server_rx,
    )
}

fn intent() -> PermissionIntent {
    let seed = PermissionIntentSeed::new(
        "Bash",
        PermissionActionDigest::from_bytes([1; 32]),
        PermissionDisplayDigest::from_bytes([2; 32]),
        PermissionSchemaDigest::from_bytes([3; 32]),
        None,
    )
    .unwrap();
    PermissionIntent::bind(seed, 4, 5, "permission-1").unwrap()
}

fn question(text: &str) -> Question {
    Question {
        question: text.into(),
        options: Vec::new(),
        allow_multiple: false,
        header: None,
    }
}

async fn acknowledge(
    connection: &Arc<Connection<loopal_ipc::connection::Listening>>,
    incoming: &mut tokio::sync::mpsc::Receiver<Incoming>,
) -> (String, serde_json::Value) {
    let Some(Incoming::Request { id, method, params }) = incoming.recv().await else {
        panic!("expected response request")
    };
    connection
        .respond(id, serde_json::json!({"resolved": true}))
        .await
        .unwrap();
    (method, params)
}

include!("tests/responses.rs");
include!("tests/event_lifecycle.rs");
