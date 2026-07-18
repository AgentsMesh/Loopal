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
const TIMEOUT: Duration = Duration::from_secs(20);

/// Which production provider adapter the spawned agent uses to reach the mock.
/// One semantic scenario drives every provider's wire; only the config differs.
#[derive(Clone, Copy, Debug)]
pub enum Provider {
    Anthropic,
    Google,
    OpenAiResponses,
    OpenAiCompat,
}

impl Provider {
    pub fn model(self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-opus-4-8",
            Provider::Google => "gemini-2.0-flash",
            Provider::OpenAiResponses => "gpt-4o",
            Provider::OpenAiCompat => "mockcompat-model",
        }
    }

    fn configure(self, cmd: &mut tokio::process::Command, home: &std::path::Path, base_url: &str) {
        match self {
            Provider::Anthropic => {
                cmd.env("ANTHROPIC_API_KEY", API_KEY)
                    .env("ANTHROPIC_BASE_URL", base_url);
            }
            Provider::Google => {
                cmd.env("GOOGLE_API_KEY", API_KEY)
                    .env("GOOGLE_BASE_URL", base_url);
            }
            Provider::OpenAiResponses => {
                cmd.env("OPENAI_API_KEY", API_KEY)
                    .env("OPENAI_BASE_URL", base_url);
            }
            Provider::OpenAiCompat => {
                let dir = home.join(".loopal");
                std::fs::create_dir_all(&dir).unwrap();
                let settings = json!({
                    "providers": {"openai_compat": [{
                        "name": "mockcompat",
                        "base_url": base_url,
                        "api_key": API_KEY,
                        "model_prefix": "mockcompat-"
                    }]}
                });
                std::fs::write(
                    dir.join("settings.json"),
                    serde_json::to_vec_pretty(&settings).unwrap(),
                )
                .unwrap();
            }
        }
    }
}

fn binary_path() -> String {
    std::env::var("LOOPAL_BINARY")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_loopal"))
        .expect("set LOOPAL_BINARY or CARGO_BIN_EXE_loopal to the loopal binary")
}

#[derive(Default, Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub thinking: String,
    pub finished: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    pub events: Vec<String>,
}

impl TurnOutcome {
    /// How many `ToolResult` events the turn produced.
    pub fn tool_result_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.starts_with("ToolResult"))
            .count()
    }
}

/// The harness's Hub-side secret vault: serve-mode agents resolve
/// `<secret_ref:NAME>` via `hub/secret/get` IPC to the stdio peer, so tests
/// stock this vault and (for outage scenarios) flip it into failing mode.
#[derive(Default)]
pub struct HarnessVault {
    entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
    failing: std::sync::atomic::AtomicBool,
    gets: std::sync::Mutex<Vec<Value>>,
}

impl HarnessVault {
    pub fn insert(&self, name: &str, plaintext: &str) {
        self.entries
            .lock()
            .unwrap()
            .insert(name.to_string(), plaintext.to_string());
    }

    /// While failing, every `hub/secret/get` returns a transport-style error
    /// (classified transient on the agent side, driving HubHealth degradation).
    pub fn set_failing(&self, failing: bool) {
        self.failing
            .store(failing, std::sync::atomic::Ordering::Release);
    }

    /// Recorded `hub/secret/get` request params, in arrival order.
    pub fn gets(&self) -> Vec<Value> {
        self.gets.lock().unwrap().clone()
    }
}

/// The harness's user seat for `agent/permission` asks. Under bypass no asks
/// arrive; ask-mode tests flip `set_allow` and read back what was asked.
/// Defaults to deny so an unexpected ask fails loudly instead of silently
/// executing a tool.
#[derive(Default)]
pub struct PermissionDesk {
    allow: std::sync::atomic::AtomicBool,
    hold: std::sync::atomic::AtomicBool,
    asks: std::sync::Mutex<Vec<Value>>,
}

impl PermissionDesk {
    pub fn set_allow(&self, allow: bool) {
        self.allow
            .store(allow, std::sync::atomic::Ordering::Release);
    }

    /// While held, asks are recorded but never answered — the user seat stays
    /// silent so a racing classifier decision must win.
    pub fn set_hold(&self, hold: bool) {
        self.hold.store(hold, std::sync::atomic::Ordering::Release);
    }

    /// Recorded `agent/permission` request params, in arrival order.
    pub fn asks(&self) -> Vec<Value> {
        self.asks.lock().unwrap().clone()
    }
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
    provider: Provider,
    mcp: bool,
    mcp_calls: Arc<std::sync::Mutex<Vec<Value>>>,
    vault: Arc<HarnessVault>,
    permissions: Arc<PermissionDesk>,
}

async fn boot_process(
    provider: Provider,
    home: &std::path::Path,
    base_url: &str,
    mcp: bool,
    mcp_calls: Arc<std::sync::Mutex<Vec<Value>>>,
    vault: Arc<HarnessVault>,
    permissions: Arc<PermissionDesk>,
) -> (
    tokio::process::Child,
    Arc<Connection<Listening>>,
    Receiver<Incoming>,
) {
    // Isolated TMPDIR: startup housekeeping cleans "orphan" bash-log dirs
    // under the shared temp root, so concurrently starting sibling test
    // processes would otherwise delete each other's live session dirs.
    let tmp = home.join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut cmd = tokio::process::Command::new(binary_path());
    cmd.arg("--serve")
        .env("HOME", home)
        .env("TMPDIR", &tmp)
        .env("LOOPAL_TEST_SESSION_DIR", home.join("sessions"))
        .env("LOOPAL_PERMISSION_MODE", "bypass")
        .env("LOOPAL_HUB_HEALTH_TICK_SECS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    provider.configure(&mut cmd, home, base_url);
    let mut child = cmd.spawn().expect("spawn loopal --serve");
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(BufReader::new(stdout)),
        Box::new(stdin),
    ));
    let (conn, raw_rx) = Connection::new(transport).into_listening();
    let rx = spawn_hub_dispatcher(conn.clone(), raw_rx, mcp, mcp_calls, vault, permissions);

    tokio::time::timeout(
        TIMEOUT,
        conn.send_request("initialize", json!({"protocol_version": 1})),
    )
    .await
    .expect("initialize timed out")
    .expect("initialize failed");

    (child, conn, rx)
}

impl CliHarness {
    pub async fn start(scenario: Value) -> Self {
        Self::spawn(scenario, Provider::Anthropic, false).await
    }

    pub async fn start_with(scenario: Value, provider: Provider) -> Self {
        Self::spawn(scenario, provider, false).await
    }

    /// Start with an MCP server configured in the agent's cwd. In `--serve`
    /// mode the agent never spawns MCP processes itself — its kernel proxies
    /// every MCP operation over IPC to the stdio peer (the Hub in production).
    /// The harness plays that Hub role: it advertises one `mcp_echo` tool and
    /// answers `hub/mcp/*` requests, so a turn exercises config discovery,
    /// proxy tool registration, and tool dispatch through the real stack.
    pub async fn start_with_mcp(scenario: Value) -> Self {
        Self::spawn(scenario, Provider::Anthropic, true).await
    }

    async fn spawn(scenario: Value, provider: Provider, mcp: bool) -> Self {
        let scenario =
            Scenario::from_slice(&serde_json::to_vec(&scenario).unwrap()).expect("valid scenario");
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let mock = tokio::spawn(serve(listener, scenario, API_KEY.to_string())).abort_handle();

        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        if mcp {
            write_mcp_settings(cwd.path());
        }
        let mcp_calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let vault = Arc::new(HarnessVault::default());
        let permissions = Arc::new(PermissionDesk::default());
        let (child, conn, rx) = boot_process(
            provider,
            home.path(),
            &base_url,
            mcp,
            mcp_calls.clone(),
            vault.clone(),
            permissions.clone(),
        )
        .await;

        Self {
            base_url,
            conn,
            rx,
            _child: child,
            mock,
            http: reqwest::Client::new(),
            cwd,
            _home: home,
            provider,
            mcp,
            mcp_calls,
            vault,
            permissions,
        }
    }

    /// Kill the agent process and boot a fresh one against the same HOME,
    /// session store, mock server, and Hub-role state — the cross-process
    /// half of a session-resume scenario.
    pub async fn restart(&mut self) {
        let _ = self._child.start_kill();
        let _ = self._child.wait().await;
        let (child, conn, rx) = boot_process(
            self.provider,
            self._home.path(),
            &self.base_url,
            self.mcp,
            self.mcp_calls.clone(),
            self.vault.clone(),
            self.permissions.clone(),
        )
        .await;
        self._child = child;
        self.conn = conn;
        self.rx = rx;
    }

    /// The harness's user seat answering `agent/permission` asks.
    pub fn permissions(&self) -> Arc<PermissionDesk> {
        self.permissions.clone()
    }

    /// `hub/mcp/call_tool` requests the agent sent to the harness's Hub role.
    pub fn mcp_calls(&self) -> Vec<Value> {
        self.mcp_calls.lock().unwrap().clone()
    }

    /// The harness's Hub-side secret vault.
    pub fn vault(&self) -> Arc<HarnessVault> {
        self.vault.clone()
    }

    /// Drain events until one whose Debug form contains `needle` arrives.
    /// Used for out-of-turn notifications (e.g. HubDegraded / HubRecovered
    /// from the health poller) that race turn boundaries.
    pub async fn await_event(&mut self, needle: &str, budget: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + budget;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(250), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && format!("{:?}", event.payload).contains(needle)
                    {
                        return true;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => return false,
                Err(_) => {}
            }
        }
        false
    }

    /// Send a prompt and collect the turn's events until Finished/Error.
    pub async fn run_turn(&mut self, prompt: &str) -> TurnOutcome {
        self.run_turn_with(prompt, json!({})).await
    }

    /// `run_turn` with extra `agent/start` params merged in (e.g.
    /// `permission_mode`, `decision_mode`, `mode`).
    pub async fn run_turn_with(&mut self, prompt: &str, extra: Value) -> TurnOutcome {
        let mut params = json!({
            "prompt": prompt,
            "model": self.provider.model(),
            "cwd": self.cwd.path().to_string_lossy(),
        });
        if let (Some(base), Value::Object(overlay)) = (params.as_object_mut(), extra) {
            base.extend(overlay);
        }
        self.conn
            .send_request(methods::AGENT_START.name, params)
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
                            AgentEventPayload::ThinkingStream { text } => {
                                out.thinking.push_str(&text)
                            }
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

    /// Start the agent in **persistent** mode (no initial prompt) and drain the
    /// startup events until it is idle. Follow-up turns are driven with
    /// `turn_via_message` / `control` — this keeps the conversation alive across
    /// turns, unlike the one-shot ephemeral `run_turn`. Returns the session id.
    pub async fn begin_persistent(&mut self) -> String {
        self.start_persistent_with(json!({})).await.0
    }

    /// Resume a persisted session in a persistent process; returns the session
    /// id plus the startup events drained on the way to idle (which is where
    /// `SessionResumed` surfaces).
    pub async fn resume_persistent(&mut self, session_id: &str) -> (String, Vec<String>) {
        self.start_persistent_with(json!({"resume": session_id}))
            .await
    }

    async fn start_persistent_with(&mut self, extra: Value) -> (String, Vec<String>) {
        let mut params = json!({
            "model": self.provider.model(),
            "cwd": self.cwd.path().to_string_lossy(),
            "lifecycle": "persistent",
        });
        if let (Some(base), Value::Object(overlay)) = (params.as_object_mut(), extra) {
            base.extend(overlay);
        }
        let resp = self
            .conn
            .send_request(methods::AGENT_START.name, params)
            .await
            .expect("agent_start persistent");
        let session_id = resp["session_id"].as_str().unwrap_or_default().to_string();
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(8), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        events.push(format!("{:?}", event.payload));
                        if matches!(event.payload, AgentEventPayload::AwaitingInput) {
                            break;
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        (session_id, events)
    }

    /// Send a follow-up user message into the persistent session and collect the
    /// resulting turn (which settles back to idle).
    pub async fn turn_via_message(&mut self, text: &str) -> TurnOutcome {
        self.message_fire(text).await;
        self.collect_persistent().await
    }

    /// Deliver a user message without collecting — for injecting input while a
    /// turn is already in flight.
    pub async fn message_fire(&self, text: &str) {
        let envelope = loopal_protocol::Envelope {
            id: uuid::Uuid::new_v4(),
            source: loopal_protocol::MessageSource::Human,
            target: "main".into(),
            content: loopal_protocol::UserContent::text_only(text),
            timestamp: chrono::Utc::now(),
            summary: None,
        };
        self.conn
            .send_request(
                methods::AGENT_MESSAGE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .expect("agent_message");
    }

    /// Send a control command (e.g. Compact) into the persistent session and collect.
    pub async fn control(&mut self, command: Value) -> TurnOutcome {
        self.conn
            .send_request(methods::AGENT_CONTROL.name, command)
            .await
            .expect("agent_control");
        self.collect_persistent().await
    }

    /// Fire a control command that does not run a turn (ModelSwitch, Clear,
    /// Rewind, …); pair with `await_event` for its effect notification.
    pub async fn control_fire(&self, command: Value) {
        self.conn
            .send_request(methods::AGENT_CONTROL.name, command)
            .await
            .expect("agent_control");
    }

    /// Collect one turn's events from a persistent session until it settles
    /// (Finished / AwaitingInput / Error).
    pub async fn collect_persistent(&mut self) -> TurnOutcome {
        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(8), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::Stream { text } => out.text.push_str(&text),
                            AgentEventPayload::ThinkingStream { text } => {
                                out.thinking.push_str(&text)
                            }
                            AgentEventPayload::Error { message } => {
                                out.error = Some(message);
                                break;
                            }
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput => {
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
                    "model": self.provider.model(),
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

enum HubReply {
    Ok(Value),
    Err(String),
    Unhandled,
}

/// The serve-mode agent treats its stdio peer as the Hub and sends it
/// agent→client requests (`hub/mcp/*`, `hub/secret/*`). Answer those inline
/// and forward notifications untouched, so collect loops never see requests
/// and the kernel's proxies settle during `agent/start` instead of timing out.
fn spawn_hub_dispatcher(
    conn: Arc<Connection<Listening>>,
    mut raw_rx: Receiver<Incoming>,
    advertise_mcp: bool,
    calls: Arc<std::sync::Mutex<Vec<Value>>>,
    vault: Arc<HarnessVault>,
    permissions: Arc<PermissionDesk>,
) -> Receiver<Incoming> {
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
        while let Some(incoming) = raw_rx.recv().await {
            match incoming {
                Incoming::Request { id, method, params } => {
                    let reply = if method == methods::AGENT_PERMISSION.name {
                        permissions.asks.lock().unwrap().push(params.clone());
                        if permissions.hold.load(std::sync::atomic::Ordering::Acquire) {
                            continue;
                        }
                        let allow = permissions.allow.load(std::sync::atomic::Ordering::Acquire);
                        HubReply::Ok(json!({"allow": allow}))
                    } else {
                        match hub_mcp_reply(&method, &params, advertise_mcp, &calls) {
                            Some(value) => HubReply::Ok(value),
                            None => hub_secret_reply(&method, &params, &vault),
                        }
                    };
                    match reply {
                        HubReply::Ok(value) => {
                            let _ = conn.respond(id, value).await;
                        }
                        HubReply::Err(message) => {
                            let _ = conn.respond_error(id, -32000, &message).await;
                        }
                        HubReply::Unhandled => {
                            let _ = conn
                                .respond_error(id, -32601, "not implemented by e2e harness")
                                .await;
                        }
                    }
                }
                note @ Incoming::Notification { .. } => {
                    if tx.send(note).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}

fn hub_secret_reply(method: &str, params: &Value, vault: &Arc<HarnessVault>) -> HubReply {
    if method == methods::HUB_SECRET_GET.name {
        vault.gets.lock().unwrap().push(params.clone());
        if vault.failing.load(std::sync::atomic::Ordering::Acquire) {
            return HubReply::Err("e2e vault outage".into());
        }
        let name = params["name"].as_str().unwrap_or_default();
        return match vault.entries.lock().unwrap().get(name) {
            Some(plaintext) => HubReply::Ok(json!({"plaintext": plaintext})),
            None => HubReply::Err(format!("e2e vault has no entry named {name}")),
        };
    }
    if method == methods::HUB_SECRET_LIST_NAMES.name {
        let names: Vec<String> = vault.entries.lock().unwrap().keys().cloned().collect();
        return HubReply::Ok(json!({"names": names}));
    }
    HubReply::Unhandled
}

fn hub_mcp_reply(
    method: &str,
    params: &Value,
    advertise: bool,
    calls: &Arc<std::sync::Mutex<Vec<Value>>>,
) -> Option<Value> {
    if method == methods::HUB_MCP_SNAPSHOT.name {
        let servers = if advertise {
            vec![json!({
                "name": "mock", "transport": "stdio", "source": "project",
                "status": "connected", "tool_count": 1,
                "resource_count": 0, "prompt_count": 0, "errors": []
            })]
        } else {
            vec![]
        };
        return Some(json!({"servers": servers}));
    }
    if method == methods::HUB_MCP_LIST_TOOLS.name {
        let tools = if advertise {
            vec![json!({
                "server": "mock", "name": "mcp_echo",
                "description": "Echo back the given text.",
                "input_schema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"]
                }
            })]
        } else {
            vec![]
        };
        return Some(json!({"tools": tools}));
    }
    if method == methods::HUB_MCP_CALL_TOOL.name {
        calls.lock().unwrap().push(params.clone());
        let text = params["args"]["text"].as_str().unwrap_or_default();
        return Some(json!({
            "content": [{"type": "text", "text": format!("mcp_echo: {text}")}],
            "is_error": false
        }));
    }
    None
}

/// Declare one MCP server in the agent-cwd project config. The command is
/// never spawned — serve-mode agents proxy MCP to the Hub peer — but the
/// entry drives config discovery, the proxy settle wait, and prompt listing.
fn write_mcp_settings(cwd: &std::path::Path) {
    let dir = cwd.join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    let settings = json!({
        "mcp_servers": {"mock": {"type": "stdio", "command": "true"}}
    });
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec_pretty(&settings).unwrap(),
    )
    .unwrap();
}
