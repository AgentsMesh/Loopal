use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;
use loopal_ipc::tcp::TcpTransport;
use loopal_meta_hub::MetaHub;

#[tokio::test]
async fn silent_registration_is_bounded_and_connection_is_closed() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_hub = meta_hub.clone();
    let server = tokio::spawn(async move {
        loopal_meta_hub::server::meta_accept_loop_with_timeout(
            listener,
            server_hub,
            "token".into(),
            Duration::from_millis(50),
        )
        .await;
    });
    let stream = TcpStream::connect(address).await.unwrap();
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(TcpTransport::new(stream));
    let (client, mut rx) = Connection::new(transport).into_listening();

    let incoming = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("client reader did not observe registration timeout");

    assert!(incoming.is_none());
    assert!(!client.is_connected());
    assert!(meta_hub.lock().await.registry.is_empty());
    server.abort();
}
