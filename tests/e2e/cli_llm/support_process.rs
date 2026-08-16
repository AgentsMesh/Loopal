use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use serde_json::{Value, json};
use tokio::io::BufReader;
use tokio::sync::mpsc::Receiver;

use super::hub_controls::{HarnessVault, PermissionDesk};
use super::hub_dispatch::spawn_hub_dispatcher;

pub const API_KEY: &str = "loopal-e2e-key";
// reason: gate jobs run both e2e suites concurrently on slower CI machines.
pub(super) const TIMEOUT: Duration = Duration::from_secs(40);

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

    fn configure(self, cmd: &mut tokio::process::Command, home: &Path, base_url: &str) {
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
                write_settings(&dir, &settings);
            }
        }
    }
}

fn binary_path() -> String {
    std::env::var("LOOPAL_BINARY")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_loopal"))
        .expect("set LOOPAL_BINARY or CARGO_BIN_EXE_loopal to the loopal binary")
}

pub(super) async fn boot_process(
    provider: Provider,
    home: &Path,
    base_url: &str,
    mcp: bool,
    mcp_calls: Arc<Mutex<Vec<Value>>>,
    vault: Arc<HarnessVault>,
    permissions: Arc<PermissionDesk>,
) -> (
    tokio::process::Child,
    Arc<Connection<Listening>>,
    Receiver<Incoming>,
) {
    // reason: shared temp cleanup can delete live sibling-test session directories.
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

pub(super) fn write_mcp_settings(cwd: &Path) {
    let dir = cwd.join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    let settings = json!({
        "mcp_servers": {"mock": {"type": "stdio", "command": "true"}}
    });
    write_settings(&dir, &settings);
}

pub(super) fn write_telemetry_settings(home: &Path) {
    let dir = home.join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    let settings = json!({
        "telemetry": {
            "enabled": true,
            "traces": true,
            "metrics": false,
            "logs": false,
            "sample_rate": 1.0,
            "file_export": true,
            "telemetry_dir": dir.join("telemetry").to_string_lossy()
        }
    });
    write_settings(&dir, &settings);
}

fn write_settings(dir: &Path, settings: &Value) {
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec_pretty(settings).unwrap(),
    )
    .unwrap();
}
