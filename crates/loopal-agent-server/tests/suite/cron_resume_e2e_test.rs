//! End-to-end resume test for durable cron tasks.
//!
//! Covers the full path:
//!   agent/start (first run) → session_dir created →
//!   external seed to `{session_dir}/{id}/cron.json` →
//!   agent/start (second run, resume=id) →
//!   session_start awaits `load_persisted` before cron_bridge.spawn →
//!   CronsChanged event carries the rehydrated task.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;
use loopal_test_support::mock_provider::MultiCallProvider;

use super::bridge_helpers::make_duplex_pair;

const T: Duration = Duration::from_secs(10);

struct Server {
    conn: Arc<Connection>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    handle: tokio::task::JoinHandle<()>,
}

async fn spawn_server(
    cwd: std::path::PathBuf,
    session_dir: std::path::PathBuf,
    calls: Vec<Vec<Result<loopal_provider_api::StreamChunk, loopal_error::LoopalError>>>,
) -> Server {
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let (server_t, client_t): (Arc<dyn Transport>, Arc<dyn Transport>) = make_duplex_pair();
    let handle = tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });
    let conn = Arc::new(Connection::new(client_t));
    let rx = conn.start();
    Server { conn, rx, handle }
}

async fn initialize_and_start(
    conn: &Connection,
    fixture: &TestFixture,
    extra: serde_json::Value,
) -> String {
    tokio::time::timeout(
        T,
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
    let resp = tokio::time::timeout(T, conn.send_request(methods::AGENT_START.name, params))
        .await
        .unwrap()
        .unwrap();
    resp["session_id"].as_str().unwrap().to_string()
}

async fn wait_for_finish(rx: &mut tokio::sync::mpsc::Receiver<Incoming>) {
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name
                    && let Ok(ev) = serde_json::from_value::<AgentEvent>(params)
                    && matches!(
                        ev.payload,
                        AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                    )
                {
                    return;
                }
            }
            _ => return,
        }
    }
}

fn seed_cron_file(path: &std::path::Path, session_id: &str) {
    let dir = path.join(session_id);
    std::fs::create_dir_all(&dir).unwrap();
    let now_ms = Utc::now().timestamp_millis();
    let seed = serde_json::json!({
        "version": 1,
        "tasks": [{
            "id": "stay1234",
            "cron": "*/5 * * * *",
            "prompt": "persisted-across-restart",
            "recurring": true,
            "created_at_unix_ms": now_ms,
            "last_fired_unix_ms": null
        }]
    });
    std::fs::write(dir.join("cron.json"), serde_json::to_vec(&seed).unwrap()).unwrap();
}

async fn collect_first_crons_changed(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
) -> Option<Vec<loopal_protocol::CronJobSnapshot>> {
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name
                    && let Ok(ev) = serde_json::from_value::<AgentEvent>(params)
                    && let AgentEventPayload::CronsChanged { crons } = ev.payload
                    && !crons.is_empty()
                {
                    return Some(crons);
                }
            }
            _ => break,
        }
    }
    None
}

/// Whole-pipeline resume test: durable cron tasks survive a process
/// restart and surface in the second session's event stream before the
/// first prompt turn completes.
#[tokio::test]
async fn durable_cron_task_rehydrated_on_resume() {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");

    // --- Phase 1: create session ---------------------------------
    let mut s1 = spawn_server(
        cwd.clone(),
        session_dir.clone(),
        vec![chunks::text_turn("hello")],
    )
    .await;
    let session_id = initialize_and_start(&s1.conn, &fixture, serde_json::json!({})).await;
    wait_for_finish(&mut s1.rx).await;
    drop(s1.conn);
    let _ = tokio::time::timeout(Duration::from_secs(2), s1.handle).await;

    // --- External seed: write durable cron tasks to disk ---------
    seed_cron_file(&session_dir, &session_id);

    // --- Phase 2: resume session ---------------------------------
    let mut s2 = spawn_server(
        cwd.clone(),
        session_dir.clone(),
        vec![chunks::text_turn("ok")],
    )
    .await;
    let _ = initialize_and_start(
        &s2.conn,
        &fixture,
        serde_json::json!({"resume": session_id}),
    )
    .await;

    let crons = collect_first_crons_changed(&mut s2.rx)
        .await
        .expect("CronsChanged must be emitted after resume");
    assert_eq!(crons.len(), 1);
    assert_eq!(crons[0].id, "stay1234");
    assert_eq!(crons[0].prompt, "persisted-across-restart");
    assert!(crons[0].durable, "rehydrated task must be flagged durable");
    drop(s2.conn);
    let _ = tokio::time::timeout(Duration::from_secs(2), s2.handle).await;
}
