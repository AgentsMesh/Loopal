use std::sync::Arc;

use loopal_agent_hub::{Hub, hub_server};
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::AgentEvent;
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

pub async fn setup(
    root: &std::path::Path,
) -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<Listening>>,
    mpsc::Receiver<Incoming>,
) {
    let (events, _rx) = mpsc::channel::<AgentEvent>(16);
    let mut instance = Hub::with_cwd(events, root.to_path_buf());
    instance.user_config_dir = Some(root.join(".loopal-user"));
    let hub = Arc::new(Mutex::new(instance));
    let (listener, port, token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    let accepted = hub.clone();
    let expected = token.clone();
    tokio::spawn(async move { hub_server::accept_loop(listener, accepted, expected).await });
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, rx) = Connection::new(transport).into_listening();
    conn.send_request(
        methods::HUB_REGISTER.name,
        json!({"name": "desktop-rpc-test", "token": token, "role": "ui_client"}),
    )
    .await
    .unwrap();
    (hub, conn, rx)
}
