use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_mock_llm_server::MockLlmServer;
use serde_json::{Value, json};
use tokio::sync::mpsc::Receiver;

use super::ui::register_ui_client;
#[path = "support_hub_handshake.rs"]
mod handshake;
#[path = "support_hub_restart.rs"]
mod restart;

pub const API_KEY: &str = "loopal-e2e-key";
// reason: the gate runs both e2e suites concurrently on slow CI machines.
pub(super) const TIMEOUT: Duration = Duration::from_secs(60);

pub struct HubEnv {
    pub home: tempfile::TempDir,
    pub cwd: tempfile::TempDir,
    pub agent_binary_override: Option<std::path::PathBuf>,
    pub permission_mode: String,
}

impl HubEnv {
    pub fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            cwd: tempfile::tempdir().unwrap(),
            agent_binary_override: None,
            permission_mode: "bypass".into(),
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
    pub(super) conn: Arc<Connection<Listening>>,
    pub(super) rx: Receiver<Incoming>,
    pub(super) hub_addr: String,
    pub(super) hub_token: String,
    _child: tokio::process::Child,
    mock: MockLlmServer,
    pub(super) _home: tempfile::TempDir,
    _cwd: tempfile::TempDir,
    agent_binary_override: Option<std::path::PathBuf>,
    permission_mode: String,
}

impl HubHarness {
    pub async fn start(scenario: Value) -> Self {
        Self::launch(HubEnv::new(), scenario, false).await
    }

    pub async fn start_with_mcp(scenario: Value) -> Self {
        Self::launch(HubEnv::new(), scenario, true).await
    }

    pub async fn launch(env: HubEnv, scenario: Value, mcp: bool) -> Self {
        Self::launch_resume(env, scenario, mcp, None).await
    }

    async fn launch_resume(
        env: HubEnv,
        scenario: Value,
        mcp: bool,
        resume: Option<String>,
    ) -> Self {
        let HubEnv {
            home,
            cwd,
            agent_binary_override,
            permission_mode,
        } = env;
        let mock = MockLlmServer::start(scenario, API_KEY).await;
        let base_url = mock.base_url().to_owned();
        let binary = binary_path();

        if mcp {
            write_hub_settings(home.path());
        }
        let tmp = home.path().join("tmp");
        std::fs::create_dir_all(&tmp).unwrap();

        let mut command = tokio::process::Command::new(&binary);
        command
            .arg("--hub-only")
            .current_dir(cwd.path())
            .env("HOME", home.path())
            .env("TMPDIR", &tmp)
            .env("LOOPAL_TEST_SESSION_DIR", home.path().join("sessions"))
            .env("LOOPAL_PERMISSION_MODE", &permission_mode)
            .env("LOOPAL_MODEL", "claude-opus-4-8")
            .env("ANTHROPIC_API_KEY", API_KEY)
            .env("ANTHROPIC_BASE_URL", &base_url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(session_id) = resume.as_ref() {
            command.arg("--resume").arg(session_id);
        }
        if let Some(agent_binary) = agent_binary_override.as_ref() {
            command
                .env("LOOPAL_BINARY", agent_binary)
                .env("LOOPAL_E2E_REAL_BINARY", &binary);
        }
        let mut child = command.spawn().expect("spawn loopal --hub-only");

        let stdout = child.stdout.take().unwrap();
        let handshake = tokio::time::timeout(TIMEOUT, handshake::find_handshake_line(stdout))
            .await
            .expect("hub handshake timed out")
            .expect("hub handshake read failed");
        let mut parts = handshake["LOOPAL_HUB ".len()..].splitn(3, ' ');
        let addr = parts.next().expect("hub addr").to_string();
        let token = parts.next().expect("hub token").to_string();
        let session_id = parts.next().expect("hub session").trim().to_string();
        if let Some(expected) = resume {
            assert_eq!(
                session_id, expected,
                "resumed Hub changed the root session id"
            );
        }

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
            _home: home,
            _cwd: cwd,
            agent_binary_override,
            permission_mode,
        };
        harness.drain_startup_backlog().await;
        harness
    }

    pub fn cwd(&self) -> &std::path::Path {
        self._cwd.path()
    }

    pub fn protected_audit_path(&self) -> std::path::PathBuf {
        self._home
            .path()
            .join(".loopal/telemetry/secret_access.jsonl")
    }

    pub async fn journal(&self) -> Value {
        self.mock.requests().await
    }
}

fn binary_path() -> std::path::PathBuf {
    loopal_agent_client::require_runfile_env("LOOPAL_BINARY").expect("resolve LOOPAL_BINARY")
}

fn write_hub_settings(home: &std::path::Path) {
    let command = loopal_agent_client::require_runfile_env("LOOPAL_MOCK_MCP_BINARY")
        .expect("resolve LOOPAL_MOCK_MCP_BINARY")
        .to_string_lossy()
        .into_owned();
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
