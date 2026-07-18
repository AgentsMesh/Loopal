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
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default, Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub finished: bool,
    pub error: Option<String>,
    pub events: Vec<String>,
}

pub struct HubHarness {
    pub base_url: String,
    pub session_id: String,
    conn: Arc<Connection<Listening>>,
    rx: Receiver<Incoming>,
    _child: tokio::process::Child,
    mock: tokio::task::AbortHandle,
    http: reqwest::Client,
    _home: tempfile::TempDir,
    _cwd: tempfile::TempDir,
}

impl HubHarness {
    /// Boot a full Hub topology with one MCP server configured; the Hub
    /// spawns the real `mock_mcp_server` subprocess itself.
    pub async fn start_with_mcp(scenario: Value) -> Self {
        let scenario =
            Scenario::from_slice(&serde_json::to_vec(&scenario).unwrap()).expect("valid scenario");
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let mock = tokio::spawn(serve(listener, scenario, API_KEY.to_string())).abort_handle();

        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        write_hub_settings(home.path());
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

        let stream = TcpStream::connect(&addr).await.expect("connect hub");
        let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
        let (conn, rx) = Connection::new(transport).into_listening();
        let response = tokio::time::timeout(
            TIMEOUT,
            conn.send_request(
                methods::HUB_REGISTER.name,
                json!({"name": "e2e-harness", "token": token, "role": "ui_client"}),
            ),
        )
        .await
        .expect("hub/register timed out")
        .expect("hub/register failed");
        assert!(
            response.get("error").is_none(),
            "hub/register rejected: {response}"
        );

        Self {
            base_url,
            session_id,
            conn,
            rx,
            _child: child,
            mock,
            http: reqwest::Client::new(),
            _home: home,
            _cwd: cwd,
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
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::Stream { text } => out.text.push_str(&text),
                            AgentEventPayload::Error { message } => {
                                out.error = Some(message);
                                break;
                            }
                            AgentEventPayload::Finished => {
                                out.finished = true;
                                break;
                            }
                            AgentEventPayload::AwaitingInput => {
                                if !out.text.is_empty() || !out.events.is_empty() {
                                    out.finished = true;
                                    break;
                                }
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
