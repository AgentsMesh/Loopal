//! Reusable cluster test harness — boots MetaHub + N Hub instances with
//! real agent processes backed by mock LLM responses.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_client::AgentProcess;
use loopal_agent_hub::{Hub, HubUplink};
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::tcp::TcpTransport;
use loopal_protocol::AgentEvent;
use serde_json::json;

use loopal_meta_hub::MetaHub;

// ── MetaHub handle ──────────────────────────────────────────────

pub struct MetaHubHandle {
    pub meta_hub: Arc<Mutex<MetaHub>>,
    pub addr: String,
    pub token: String,
}

impl MetaHubHandle {
    /// Boot a MetaHub TCP listener on localhost random port.
    pub async fn boot() -> Self {
        let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
        let (listener, token) = loopal_meta_hub::server::start_meta_listener("127.0.0.1:0")
            .await
            .expect("bind MetaHub");
        let addr = listener.local_addr().unwrap().to_string();
        let mh = meta_hub.clone();
        let t = token.clone();
        tokio::spawn(async move {
            loopal_meta_hub::server::meta_accept_loop(listener, mh, t).await;
        });
        Self {
            meta_hub,
            addr,
            token,
        }
    }
}

// ── Hub handle ──────────────────────────────────────────────────

#[allow(dead_code)]
pub struct HubHandle {
    pub name: String,
    pub hub: Arc<Mutex<Hub>>,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub agent_proc: AgentProcess,
    pub root_conn: Arc<Connection>,
}

impl HubHandle {
    /// Boot a Hub with a real agent process, connected to MetaHub.
    pub async fn boot(name: &str, meta: &MetaHubHandle) -> Self {
        // Create Hub
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(256);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));

        // Connect to MetaHub via TCP
        let stream = tokio::net::TcpStream::connect(&meta.addr)
            .await
            .expect("TCP connect to MetaHub");
        let transport: Arc<dyn loopal_ipc::transport::Transport> =
            Arc::new(TcpTransport::new(stream));
        let conn = Arc::new(Connection::new(transport));
        let rx = conn.start();

        // meta/register handshake
        let resp = conn
            .send_request(
                methods::META_REGISTER.name,
                json!({"name": name, "token": meta.token, "capabilities": []}),
            )
            .await
            .expect("meta/register");
        assert_eq!(resp["ok"].as_bool(), Some(true));

        // Set uplink + reverse handler
        let uplink = Arc::new(HubUplink::new(conn.clone(), name.into()));
        hub.lock().await.uplink = Some(uplink);
        let rh = hub.clone();
        let rc = conn;
        let rn = name.to_string();
        tokio::spawn(async move {
            loopal_agent_hub::uplink::handle_reverse_requests(rh, rc, rx, rn).await;
        });

        // Spawn real agent process with mock provider
        let mock_file = create_mock_fixture(name);
        let exe = resolve_binary();
        let agent_proc = AgentProcess::spawn_with_env(
            Some(&exe),
            &[("LOOPAL_TEST_PROVIDER", mock_file.to_str().unwrap())],
        )
        .await
        .expect("spawn agent");

        let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
        client.initialize().await.expect("initialize");
        let cwd = std::env::temp_dir();
        client
            .start_agent(&cwd, None, Some("act"), None, None, true, None, None)
            .await
            .expect("start_agent");

        // Register in Hub
        let (root_conn, incoming_rx) = client.into_parts();
        loopal_agent_hub::agent_io::start_agent_io(
            hub.clone(),
            "main",
            root_conn.clone(),
            incoming_rx,
        );

        // Start event loop
        let _event_loop = loopal_agent_hub::start_event_loop(hub.clone(), event_rx);
        let (_, fresh_rx) = mpsc::channel::<AgentEvent>(256);

        Self {
            name: name.to_string(),
            hub,
            event_rx: fresh_rx,
            agent_proc,
            root_conn,
        }
    }

    /// Wait for AwaitingInput event (agent is ready).
    #[allow(dead_code)]
    pub async fn wait_ready(&mut self, event_rx: &mut mpsc::Receiver<AgentEvent>) {
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            while let Ok(event) = event_rx.try_recv() {
                if matches!(
                    event.payload,
                    loopal_protocol::AgentEventPayload::AwaitingInput
                ) {
                    return;
                }
            }
        }
        panic!("agent {} never became ready", self.name);
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn create_mock_fixture(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal_cluster_mock_{}_{}.json",
        name,
        std::process::id()
    ));
    let data = json!([
        [
            {"type": "text", "text": format!("Hello from {name} mock!")},
            {"type": "usage", "input": 10, "output": 5},
            {"type": "done"}
        ]
    ]);
    std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
    path
}

fn resolve_binary() -> String {
    if let Ok(path) = std::env::var("LOOPAL_BINARY") {
        if std::path::Path::new(&path).exists() {
            return path;
        }
    }
    let test_exe = std::env::current_exe().expect("current_exe");
    let target_dir = test_exe
        .parent()
        .and_then(|p| p.parent())
        .expect("target dir");
    let binary = format!("loopal{}", std::env::consts::EXE_SUFFIX);
    let path = target_dir.join(binary);
    assert!(
        path.exists(),
        "loopal binary not found at {}",
        path.display()
    );
    path.to_string_lossy().to_string()
}
