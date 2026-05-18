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
async fn rpc_error_payload_constructors() {
    let internal = RpcErrorPayload::internal("oops");
    assert_eq!(internal.code, loopal_ipc::jsonrpc::INTERNAL_ERROR);
    assert_eq!(internal.message, "oops");

    let nf = RpcErrorPayload::method_not_found("missing");
    assert_eq!(nf.code, loopal_ipc::jsonrpc::METHOD_NOT_FOUND);
    assert_eq!(nf.message, "missing");
}
