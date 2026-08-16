use super::*;

#[tokio::test]
async fn response_error_closes_transport() {
    let transport = TestTransport::recording();
    let connection = test_connection(transport.clone());

    let keep_connection = super::super::respond_bounded(
        &connection,
        std::future::ready(Err(RpcError::Transport("failed".into()))),
    )
    .await;

    assert!(!keep_connection);
    assert!(transport.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn response_timeout_closes_transport() {
    let transport = TestTransport::recording();
    let connection = test_connection(transport.clone());

    let keep_connection =
        super::super::respond_bounded(&connection, pending::<Result<(), RpcError>>()).await;

    assert!(!keep_connection);
    assert!(transport.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn transport_close_timeout_is_bounded() {
    let transport = TestTransport::blocking_close();
    let connection = test_connection(transport.clone());

    tokio::time::timeout(
        Duration::from_millis(500),
        super::super::close_response_transport(&connection),
    )
    .await
    .expect("transport close must respect the UI response deadline");

    assert!(!transport.closed.load(Ordering::Acquire));
}
