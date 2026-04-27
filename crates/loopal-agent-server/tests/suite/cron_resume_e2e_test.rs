//! End-to-end resume test for session-scoped cron tasks via `agent/start`.
//!
//! Covers the startup-resume path:
//!   agent/start (first run) → session dir created →
//!   external seed to `{sessions_root}/{id}/cron.json` →
//!   agent/start (second run with resume=id) →
//!   session_start binds the scheduler to the resumed session id (which
//!   loads the seed) before `cron_bridge.spawn` →
//!   CronsChanged event carries the rehydrated task.
//!
//! Companion test `cron_follows_control_resume_test` covers the
//! `ControlCommand::ResumeSession` path (in-process session swap).
//! Shared scaffolding lives in `resume_e2e_helpers`.

use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;

use super::resume_e2e_helpers::{
    CronSeed, initialize_and_start, seed_cron_file, spawn_resume_server, wait_for_event,
    wait_for_idle,
};

/// Whole-pipeline resume test: durable cron tasks survive a process
/// restart and surface in the second session's event stream before the
/// first prompt turn completes.
#[tokio::test]
async fn durable_cron_task_rehydrated_on_resume() {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");

    // --- Phase 1: create session ---------------------------------
    let mut s1 = spawn_resume_server(
        cwd.clone(),
        session_dir.clone(),
        vec![chunks::text_turn("hello")],
    )
    .await;
    let session_id = initialize_and_start(&s1.conn, &fixture, serde_json::json!({})).await;
    wait_for_idle(&mut s1.rx).await;
    drop(s1.conn);
    let _ = tokio::time::timeout(Duration::from_secs(2), s1.handle).await;

    // --- External seed: write durable cron tasks to disk ---------
    seed_cron_file(
        &session_dir,
        &session_id,
        &CronSeed::with_defaults("stay1234"),
    );

    // --- Phase 2: resume session ---------------------------------
    let mut s2 = spawn_resume_server(
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

    let payload = wait_for_event(
        &mut s2.rx,
        |p| matches!(p, AgentEventPayload::CronsChanged { crons } if !crons.is_empty()),
    )
    .await
    .expect("CronsChanged must be emitted after resume");
    let AgentEventPayload::CronsChanged { crons } = payload else {
        unreachable!();
    };
    assert_eq!(crons.len(), 1);
    assert_eq!(crons[0].id, "stay1234");
    assert_eq!(crons[0].prompt, "persisted-across-restart");
    assert!(crons[0].durable, "rehydrated task must be flagged durable");
    drop(s2.conn);
    let _ = tokio::time::timeout(Duration::from_secs(2), s2.handle).await;
}
