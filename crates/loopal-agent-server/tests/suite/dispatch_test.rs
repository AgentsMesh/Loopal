use loopal_agent_server::dispatch::{RpcErrorPayload, dispatch_simple};
use loopal_agent_server::session_hub::SessionHub;
use loopal_ipc::protocol::methods;

#[tokio::test]
async fn dispatch_simple_lists_sessions() {
    let hub = SessionHub::new();
    let outcome = dispatch_simple(methods::AGENT_LIST.name, &hub).await;
    let value = outcome.expect("list ok");
    assert!(value.is_array(), "got: {value}");
}

#[tokio::test]
async fn dispatch_simple_shutdown_is_ok() {
    let hub = SessionHub::new();
    let outcome = dispatch_simple(methods::AGENT_SHUTDOWN.name, &hub).await;
    let value = outcome.expect("shutdown ok");
    assert_eq!(value["ok"], true);
}

#[tokio::test]
async fn dispatch_simple_unknown_method_returns_method_not_found() {
    let hub = SessionHub::new();
    let outcome = dispatch_simple("agent/totally_made_up", &hub).await;
    let err = outcome.expect_err("unknown method should error");
    assert_eq!(err.code, loopal_ipc::jsonrpc::METHOD_NOT_FOUND);
    assert!(
        err.message.contains("totally_made_up"),
        "msg: {}",
        err.message
    );
}

#[tokio::test]
async fn dispatch_simple_initialize_is_idempotent() {
    // The canonical first `initialize` is consumed by
    // `wait_for_initialize_with_token`. A client that hits its initialize
    // timeout will retry with a new request id, and that retry MUST land here
    // and succeed — otherwise the connection appears broken (-32601) even
    // though the agent is healthy. Guards against the bazel-sandbox e2e
    // failure where slow child stdin caused legitimate retries to explode.
    let hub = SessionHub::new();

    let first = dispatch_simple(methods::INITIALIZE.name, &hub)
        .await
        .expect("first initialize must succeed");
    let second = dispatch_simple(methods::INITIALIZE.name, &hub)
        .await
        .expect("second initialize must also succeed (idempotent)");

    assert_eq!(
        first, second,
        "idempotent initialize must return same result"
    );
    assert_eq!(first["protocol_version"], 1);
    assert_eq!(first["agent_info"]["name"], "loopal");
    assert!(
        first["agent_info"]["version"].is_string(),
        "version must be present, got: {first}"
    );
}

#[tokio::test]
async fn rpc_error_payload_constructors() {
    let internal = RpcErrorPayload::internal("oops");
    assert_eq!(internal.code, loopal_ipc::jsonrpc::INTERNAL_ERROR);
    assert_eq!(internal.message, "oops");

    let nf = RpcErrorPayload::method_not_found("missing");
    assert_eq!(nf.code, loopal_ipc::jsonrpc::METHOD_NOT_FOUND);
    assert_eq!(nf.message, "missing");
}
