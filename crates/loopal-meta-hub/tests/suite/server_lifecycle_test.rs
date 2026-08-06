use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore, mpsc};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::tcp::TcpTransport;
use loopal_ipc::transport::Transport;
use loopal_meta_hub::MetaHub;

struct PendingSendTransport {
    closed: AtomicBool,
    send_started: Semaphore,
}

#[async_trait]
impl Transport for PendingSendTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        self.send_started.add_permits(1);
        std::future::pending().await
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        std::future::pending().await
    }

    fn is_connected(&self) -> bool {
        !self.closed.load(Ordering::Acquire)
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

fn blocked_connection() -> (Arc<Connection<Listening>>, Arc<PendingSendTransport>) {
    let transport = Arc::new(PendingSendTransport {
        closed: AtomicBool::new(false),
        send_started: Semaphore::new(0),
    });
    let (connection, _rx) = Connection::new(transport.clone()).into_listening();
    (connection, transport)
}

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

#[tokio::test]
async fn blocked_registration_ack_never_activates_or_leaks_name_reservation() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (connection, transport) = blocked_connection();
    let register_hub = meta_hub.clone();
    let register_conn = connection.clone();
    let registration = tokio::spawn(async move {
        loopal_meta_hub::server::register_acknowledged_connection(
            &register_hub,
            &register_conn,
            1,
            "blocked-hub",
            vec!["desktop".into()],
            Duration::from_millis(50),
        )
        .await
    });

    transport.send_started.acquire().await.unwrap().forget();
    assert!(
        meta_hub.lock().await.registry.is_empty(),
        "a name reservation must not be visible as an active Hub"
    );

    let error = registration.await.unwrap().unwrap_err();
    assert!(error.contains("acknowledgement timed out"));
    assert!(transport.closed.load(Ordering::Acquire));

    let (replacement_transport, _) = loopal_ipc::duplex_pair();
    let (replacement, _rx) = Connection::new(replacement_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register("blocked-hub", replacement, vec![])
        .expect("failed ACK must release the reserved name");
}

#[tokio::test(start_paused = true)]
async fn blocked_io_response_is_bounded_then_closes_and_unregisters_connection() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (connection, transport) = blocked_connection();
    meta_hub
        .lock()
        .await
        .registry
        .register("blocked-hub", connection.clone(), vec![])
        .unwrap();
    let (incoming_tx, incoming_rx) = mpsc::channel(1);
    let io_hub = meta_hub.clone();
    let io_connection = connection.clone();
    let io = tokio::spawn(async move {
        loopal_meta_hub::io_loop::meta_hub_io_loop(
            io_hub,
            io_connection,
            incoming_rx,
            "blocked-hub".into(),
        )
        .await;
    });
    incoming_tx
        .send(Incoming::Request {
            id: 7,
            method: methods::META_HEARTBEAT.name.into(),
            params: serde_json::json!({"agent_count": 1}),
        })
        .await
        .unwrap();
    transport.send_started.acquire().await.unwrap().forget();

    tokio::time::advance(Duration::from_secs(3)).await;
    io.await.unwrap();
    assert!(transport.closed.load(Ordering::Acquire));
    assert!(meta_hub.lock().await.registry.is_empty());
}
