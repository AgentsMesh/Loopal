//! Full-stack CLI behavior-test harness: a real `loopal --serve` agent process,
//! driven over stdio IPC, talking to an in-process mock LLM HTTP server through
//! the production Anthropic adapter (via `ANTHROPIC_BASE_URL`). Exercises
//! bootstrap → agent loop → real provider adapter → HTTP/SSE wire → mock, and
//! lets tests assert both the agent's output and what it sent (mock journal).

#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_mock_llm_lib::{Scenario, serve};
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::{Value, json};
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Receiver;

pub const API_KEY: &str = "loopal-e2e-key";
pub const MODEL: &str = "claude-opus-4-8";
const TIMEOUT: Duration = Duration::from_secs(20);

fn binary_path() -> String {
    std::env::var("LOOPAL_BINARY")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_loopal"))
        .expect("set LOOPAL_BINARY or CARGO_BIN_EXE_loopal to the loopal binary")
}

#[derive(Default, Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub finished: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    pub events: Vec<String>,
}

pub struct CliHarness {
    pub base_url: String,
    conn: Arc<Connection<Listening>>,
    rx: Receiver<Incoming>,
    _child: tokio::process::Child,
    mock: tokio::task::AbortHandle,
    http: reqwest::Client,
    cwd: tempfile::TempDir,
    _home: tempfile::TempDir,
}

impl CliHarness {
    pub async fn start(scenario: Value) -> Self {
        let scenario =
            Scenario::from_slice(&serde_json::to_vec(&scenario).unwrap()).expect("valid scenario");
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let mock = tokio::spawn(serve(listener, scenario, API_KEY.to_string())).abort_handle();

        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let mut child = tokio::process::Command::new(binary_path())
            .arg("--serve")
            .env("HOME", home.path())
            .env("LOOPAL_TEST_SESSION_DIR", home.path().join("sessions"))
            .env("ANTHROPIC_API_KEY", API_KEY)
            .env("ANTHROPIC_BASE_URL", &base_url)
            .env("LOOPAL_PERMISSION_MODE", "bypass")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn loopal --serve");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
            Box::new(BufReader::new(stdout)),
            Box::new(stdin),
        ));
        let (conn, rx) = Connection::new(transport).into_listening();

        tokio::time::timeout(
            TIMEOUT,
            conn.send_request("initialize", json!({"protocol_version": 1})),
        )
        .await
        .expect("initialize timed out")
        .expect("initialize failed");

        Self {
            base_url,
            conn,
            rx,
            _child: child,
            mock,
            http: reqwest::Client::new(),
            cwd,
            _home: home,
        }
    }

    /// Send a prompt and collect the turn's events until Finished/Error.
    pub async fn run_turn(&mut self, prompt: &str) -> TurnOutcome {
        self.conn
            .send_request(
                methods::AGENT_START.name,
                json!({
                    "prompt": prompt,
                    "model": MODEL,
                    "cwd": self.cwd.path().to_string_lossy(),
                }),
            )
            .await
            .expect("agent_start");

        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(8), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params })) => {
                    if method == methods::AGENT_EVENT.name
                        && let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                    {
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

    /// Start a turn without collecting it to completion — for interruption tests.
    pub async fn begin_turn(&self, prompt: &str) {
        self.conn
            .send_request(
                methods::AGENT_START.name,
                json!({
                    "prompt": prompt,
                    "model": MODEL,
                    "cwd": self.cwd.path().to_string_lossy(),
                }),
            )
            .await
            .expect("agent_start");
    }

    /// Signal the running turn to cancel (`agent/interrupt` over the live wire).
    pub async fn interrupt(&self) {
        self.conn
            .send_notification(methods::AGENT_INTERRUPT.name, json!({}))
            .await
            .expect("interrupt");
    }

    /// Collect events until the turn settles (cancelled / finished / error), used
    /// after an interrupt to assert the in-flight turn was actually cancelled.
    pub async fn await_settled(&mut self, budget: Duration) -> TurnOutcome {
        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + budget;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(1), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::TurnCancelled { .. }
                            | AgentEventPayload::Interrupted => {
                                out.cancelled = true;
                                break;
                            }
                            AgentEventPayload::AwaitingInput => {
                                // cancelled turns return to idle without Finishing.
                                out.cancelled = !out.finished;
                                break;
                            }
                            AgentEventPayload::Finished => {
                                out.finished = true;
                                break;
                            }
                            AgentEventPayload::Error { message } => {
                                out.error = Some(message);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                // inner timeout — keep waiting until the outer deadline
                Err(_) => {}
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

    /// The mock's verification snapshot (`GET /__mock/verify`).
    pub async fn verify(&self) -> Value {
        self.http
            .get(format!("{}/__mock/verify", self.base_url))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        self.mock.abort();
    }
}
