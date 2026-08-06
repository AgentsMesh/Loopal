//! Full-topology behavior-test harness: a real `loopal --hub-only` process
//! (Hub + root Agent child), driven over the Hub's TCP attach protocol as a
//! registered UI client, with the LLM pointed at an in-process mock server.
//! Unlike the serve-mode suite, the Hub here actually OWNS its resources —
//! it spawns real MCP subprocesses (`LocalMcpProvider`) from settings.

#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_mock_llm_lib::{Scenario, serve};
use loopal_protocol::{
    AgentEvent, AgentEventPayload, Envelope, MessageSource, ROOT_AGENT_NAME, UserContent,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Receiver;

pub const API_KEY: &str = "loopal-e2e-key";
// reason: generous — the gate job runs both e2e suites concurrently and CI
// machines are slow; a green run never comes near this, it only pads the
// failure verdict under co-load.
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Default, Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub finished: bool,
    pub error: Option<String>,
    pub events: Vec<String>,
}

/// Pre-spawn environment for a Hub topology: create it first, stage files
/// (vault, SSH identity, project fixtures) into `home`/`cwd`, then `launch`.
pub struct HubEnv {
    pub home: tempfile::TempDir,
    pub cwd: tempfile::TempDir,
}

impl HubEnv {
    pub fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            cwd: tempfile::tempdir().unwrap(),
        }
    }
}

impl Default for HubEnv {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HubHarness {
    pub base_url: String,
    pub session_id: String,
    conn: Arc<Connection<Listening>>,
    rx: Receiver<Incoming>,
    hub_addr: String,
    hub_token: String,
    _child: tokio::process::Child,
    mock: tokio::task::AbortHandle,
    http: reqwest::Client,
    _home: tempfile::TempDir,
    _cwd: tempfile::TempDir,
}

impl HubHarness {
    pub async fn start(scenario: Value) -> Self {
        Self::launch(HubEnv::new(), scenario, false).await
    }

    /// Boot a full Hub topology with one MCP server configured; the Hub
    /// spawns the real `mock_mcp_server` subprocess itself.
    pub async fn start_with_mcp(scenario: Value) -> Self {
        Self::launch(HubEnv::new(), scenario, true).await
    }

    pub async fn launch(env: HubEnv, scenario: Value, mcp: bool) -> Self {
        let HubEnv { home, cwd } = env;
        let scenario =
            Scenario::from_slice(&serde_json::to_vec(&scenario).unwrap()).expect("valid scenario");
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let mock = tokio::spawn(serve(listener, scenario, API_KEY.to_string())).abort_handle();

        if mcp {
            write_hub_settings(home.path());
        }
        let tmp = home.path().join("tmp");
        std::fs::create_dir_all(&tmp).unwrap();

        let mut child = tokio::process::Command::new(binary_path())
            .arg("--hub-only")
            .current_dir(cwd.path())
            .env("HOME", home.path())
            .env("TMPDIR", &tmp)
            .env("LOOPAL_TEST_SESSION_DIR", home.path().join("sessions"))
            .env("LOOPAL_PERMISSION_MODE", "bypass")
            .env("LOOPAL_MODEL", "claude-opus-4-8")
            .env("ANTHROPIC_API_KEY", API_KEY)
            .env("ANTHROPIC_BASE_URL", &base_url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn loopal --hub-only");

        let stdout = child.stdout.take().unwrap();
        let handshake = tokio::time::timeout(TIMEOUT, find_handshake_line(stdout))
            .await
            .expect("hub handshake timed out")
            .expect("hub handshake read failed");
        let mut parts = handshake["LOOPAL_HUB ".len()..].splitn(3, ' ');
        let addr = parts.next().expect("hub addr").to_string();
        let token = parts.next().expect("hub token").to_string();
        let session_id = parts.next().expect("hub session").trim().to_string();

        let (conn, rx) = register_ui_client(&addr, &token, "e2e-harness").await;

        let mut harness = Self {
            base_url,
            session_id,
            conn,
            rx,
            hub_addr: addr,
            hub_token: token,
            _child: child,
            mock,
            http: reqwest::Client::new(),
            _home: home,
            _cwd: cwd,
        };
        harness.drain_startup_backlog().await;
        harness
    }

    /// The Hub's working directory (the project the root agent runs in).
    pub fn cwd(&self) -> &std::path::Path {
        self._cwd.path()
    }

    /// Attach one more UI client to the same Hub — an observer that receives
    /// the same broadcast event stream.
    pub async fn second_client(&self, name: &str) -> ObserverClient {
        let (conn, rx) = register_ui_client(&self.hub_addr, &self.hub_token, name).await;
        let mut observer = ObserverClient { _conn: conn, rx };
        observer.drain_backlog().await;
        observer
    }

    /// A freshly registered UI client receives the session's replayed startup
    /// events; consume them up to the first idle so `turn` never mistakes the
    /// backlog's AwaitingInput for a turn boundary.
    async fn drain_startup_backlog(&mut self) {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(1500), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && matches!(event.payload, AgentEventPayload::AwaitingInput)
                    {
                        return;
                    }
                }
                Ok(Some(_)) => {}
                // quiet channel: whatever backlog existed has been replayed
                Ok(None) | Err(_) => return,
            }
        }
    }

    /// Route a user message to the root agent and collect the turn.
    pub async fn turn(&mut self, text: &str) -> TurnOutcome {
        let envelope = Envelope::new(
            MessageSource::Human,
            ROOT_AGENT_NAME,
            UserContent::text_only(text),
        );
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .expect("hub/route");

        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(10), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        // Sub-agent events are broadcast on the same stream;
                        // only the ROOT agent's terminal events end the turn.
                        let root = event
                            .agent_name
                            .as_ref()
                            .map(|a| format!("{a:?}").contains(ROOT_AGENT_NAME))
                            .unwrap_or(true);
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::Stream { text } if root => out.text.push_str(&text),
                            AgentEventPayload::Error { message } if root => {
                                out.error = Some(message);
                                break;
                            }
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                                if root =>
                            {
                                out.finished = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        out
    }

    /// The mock's redacted request journal (`GET /__mock/requests`).
    pub async fn journal(&self) -> Value {
        self.http
            .get(format!("{}/__mock/requests", self.base_url))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

impl Drop for HubHarness {
    fn drop(&mut self) {
        self.mock.abort();
    }
}

/// A second registered UI client: read-only observer of the Hub's broadcast
/// event stream.
pub struct ObserverClient {
    _conn: Arc<Connection<Listening>>,
    rx: Receiver<Incoming>,
}

impl ObserverClient {
    async fn drain_backlog(&mut self) {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(1500), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && matches!(event.payload, AgentEventPayload::AwaitingInput)
                    {
                        return;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => return,
            }
        }
    }

    /// Collect broadcast events until the root agent settles or the budget
    /// runs out. Quiet gaps do NOT end collection — under co-load a turn can
    /// stall for seconds between events.
    pub async fn collect_until_settled(&mut self, budget: Duration) -> Vec<String> {
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + budget;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(2), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        events.push(format!("{:?}", event.payload));
                        if matches!(
                            event.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        ) {
                            break;
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {}
            }
        }
        events
    }
}

async fn register_ui_client(
    addr: &str,
    token: &str,
    name: &str,
) -> (Arc<Connection<Listening>>, Receiver<Incoming>) {
    let stream = TcpStream::connect(addr).await.expect("connect hub");
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, rx) = Connection::new(transport).into_listening();
    let response = tokio::time::timeout(
        TIMEOUT,
        conn.send_request(
            methods::HUB_REGISTER.name,
            json!({
                "name": name,
                "token": token,
                "role": "ui_client",
                "capabilities": {
                    "permission": false,
                    "question": false,
                    "plan_approval": false,
                },
            }),
        ),
    )
    .await
    .expect("hub/register timed out")
    .expect("hub/register failed");
    assert!(
        response.get("error").is_none(),
        "hub/register rejected: {response}"
    );
    (conn, rx)
}

fn binary_path() -> String {
    let path = std::env::var("LOOPAL_BINARY").expect("LOOPAL_BINARY env required");
    // The harness spawns with a different current_dir, so the bazel-relative
    // rootpath must be absolutized first.
    std::fs::canonicalize(&path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(path)
}

/// Global settings for the Hub: one stdio MCP server pointing at the hermetic
/// mock binary. The Hub spawns and owns this subprocess — the exact resource
/// topology production uses.
fn write_hub_settings(home: &std::path::Path) {
    let command = std::env::var("LOOPAL_MOCK_MCP_BINARY")
        .expect("LOOPAL_MOCK_MCP_BINARY env required (bazel data dep)");
    let command = std::fs::canonicalize(&command)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(command);
    let dir = home.join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    let settings = json!({
        "mcp_servers": {"mock": {"type": "stdio", "command": command}}
    });
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec_pretty(&settings).unwrap(),
    )
    .unwrap();
}

async fn find_handshake_line(stdout: tokio::process::ChildStdout) -> std::io::Result<String> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(std::io::Error::other("hub stdout closed before handshake"));
        }
        if line.starts_with("LOOPAL_HUB ") {
            return Ok(line.trim_end().to_string());
        }
    }
}
