//! Shared scaffolding for the cron-resume e2e tests.
//!
//! `cron_resume_e2e_test` and `cron_follows_control_resume_test` both
//! drive a real `run_server_for_test` instance; this module factors
//! out the boilerplate (server spawn, IPC handshake, cron seed,
//! `CronsChanged` event polling) so neither test re-implements it.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;

use super::bridge_helpers::make_duplex_pair;

pub const RESUME_E2E_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ResumeServer {
    pub conn: Arc<Connection>,
    pub rx: tokio::sync::mpsc::Receiver<Incoming>,
    pub handle: tokio::task::JoinHandle<()>,
}

pub async fn spawn_resume_server(
    cwd: std::path::PathBuf,
    session_dir: std::path::PathBuf,
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> ResumeServer {
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let (server_t, client_t): (Arc<dyn Transport>, Arc<dyn Transport>) = make_duplex_pair();
    let handle = tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });
    let conn = Arc::new(Connection::new(client_t));
    let rx = conn.start();
    ResumeServer { conn, rx, handle }
}

pub async fn initialize_and_start(
    conn: &Connection,
    fixture: &TestFixture,
    extra: serde_json::Value,
) -> String {
    tokio::time::timeout(
        RESUME_E2E_TIMEOUT,
        conn.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();
    let mut params = serde_json::json!({
        "prompt": "hi",
        "cwd": fixture.path().to_string_lossy().as_ref(),
    });
    if let serde_json::Value::Object(map) = extra {
        for (k, v) in map {
            params[k] = v;
        }
    }
    let resp = tokio::time::timeout(
        RESUME_E2E_TIMEOUT,
        conn.send_request(methods::AGENT_START.name, params),
    )
    .await
    .unwrap()
    .unwrap();
    resp["session_id"].as_str().unwrap().to_string()
}

pub async fn wait_for_event<F>(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    mut f: F,
) -> Option<AgentEventPayload>
where
    F: FnMut(&AgentEventPayload) -> bool,
{
    let deadline = tokio::time::Instant::now() + RESUME_E2E_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name
                    && let Ok(ev) = serde_json::from_value::<AgentEvent>(params)
                    && f(&ev.payload)
                {
                    return Some(ev.payload);
                }
            }
            _ => break,
        }
    }
    None
}

pub async fn wait_for_idle(rx: &mut tokio::sync::mpsc::Receiver<Incoming>) {
    let _ = wait_for_event(rx, |p| {
        matches!(
            p,
            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
        )
    })
    .await;
}

/// Parameters for `seed_cron_file`. Each field maps directly onto the
/// underlying `PersistedTask` schema; tests no longer share a hard-coded
/// cron / prompt / recurring triple.
pub struct CronSeed<'a> {
    pub task_id: &'a str,
    pub cron: &'a str,
    pub prompt: &'a str,
    pub recurring: bool,
}

impl<'a> CronSeed<'a> {
    /// Sensible defaults used by the original e2e tests: every-5-min
    /// recurring task with a fixed prompt. Override individual fields
    /// when the test cares about a specific value.
    pub fn with_defaults(task_id: &'a str) -> Self {
        Self {
            task_id,
            cron: "*/5 * * * *",
            prompt: "persisted-across-restart",
            recurring: true,
        }
    }
}

pub fn seed_cron_file(sessions_root: &std::path::Path, session_id: &str, seed: &CronSeed<'_>) {
    let dir = sessions_root.join(session_id);
    std::fs::create_dir_all(&dir).unwrap();
    let now_ms = Utc::now().timestamp_millis();
    let payload = serde_json::json!({
        "version": 1,
        "tasks": [{
            "id": seed.task_id,
            "cron": seed.cron,
            "prompt": seed.prompt,
            "recurring": seed.recurring,
            "created_at_unix_ms": now_ms,
            "last_fired_unix_ms": null
        }]
    });
    std::fs::write(dir.join("cron.json"), serde_json::to_vec(&payload).unwrap()).unwrap();
}
