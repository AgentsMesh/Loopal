//! End-to-end coverage of the user-reported bug path:
//! `ControlCommand::ResumeSession` should make `CronScheduler` follow
//! the agent across an in-process session swap.
//!
//! Distinct from `cron_resume_e2e_test`, which exercises the
//! startup-time `agent/start --resume` path. This test:
//!   1. Starts a server and lets it create session A.
//!   2. Externally seeds A's `cron.json` with one task.
//!   3. Sends `agent/control ResumeSession(A)` — the path TUI takes
//!      when the user runs `/resume <id>` after launch.
//!   4. Asserts the next `CronsChanged` event carries A's task.
//!
//! Shared scaffolding lives in `resume_e2e_helpers`.

use std::time::Duration;

use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEventPayload, ControlCommand};
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;

use super::resume_e2e_helpers::{
    CronSeed, RESUME_E2E_TIMEOUT, initialize_and_start, seed_cron_file, spawn_resume_server,
    wait_for_event,
};

/// User-reported scenario: launch Loopal in a fresh session, then issue
/// `/resume <id>` to switch to a session whose `cron.json` lives on
/// disk. The cron must reappear in the event stream — proving the
/// `SessionResumeHook` chain wires through to the file backend.
#[tokio::test]
async fn cron_follows_session_swap_via_control_command() {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");

    // Persistent lifecycle: server stays alive after the seed prompt
    // finishes, so we can drive it with a follow-up control command.
    let mut server = spawn_resume_server(
        cwd.clone(),
        session_dir.clone(),
        vec![chunks::text_turn("hello")],
    )
    .await;
    let _primary_id = initialize_and_start(
        &server.conn,
        &fixture,
        serde_json::json!({"lifecycle": "persistent"}),
    )
    .await;

    // Pre-create a target session and seed its cron.json so resume has
    // something to load.
    use loopal_runtime::SessionManager;
    let sm = SessionManager::with_base_dir(session_dir.clone());
    let target = sm
        .create_session(&cwd, "claude-sonnet-4-20250514")
        .expect("create target session");
    seed_cron_file(
        &session_dir,
        &target.id,
        &CronSeed::with_defaults("stay1234"),
    );

    // Send the same control command the TUI's `/resume` issues.
    let cmd = ControlCommand::ResumeSession(target.id.clone());
    let value = serde_json::to_value(&cmd).unwrap();
    tokio::time::timeout(
        RESUME_E2E_TIMEOUT,
        server.conn.send_request(methods::AGENT_CONTROL.name, value),
    )
    .await
    .unwrap()
    .unwrap();

    // CronsChanged itself is the authoritative signal that the scheduler
    // reloaded — with the broadcast-driven cron_bridge, it fires as soon
    // as `switch_session` completes inside the resume hook chain. No
    // additional sync barrier is needed (and adding one risks consuming
    // the very CronsChanged we're waiting for, since `wait_for_event`
    // discards non-matching events).
    let payload = wait_for_event(
        &mut server.rx,
        |p| matches!(p, AgentEventPayload::CronsChanged { crons } if !crons.is_empty()),
    )
    .await
    .expect("CronsChanged with at least one task must arrive after resume");
    let AgentEventPayload::CronsChanged { crons } = payload else {
        unreachable!();
    };
    assert_eq!(crons.len(), 1, "expected exactly the seeded task");
    assert_eq!(crons[0].id, "stay1234");
    assert_eq!(crons[0].prompt, "persisted-across-restart");
    assert!(crons[0].durable);

    drop(server.conn);
    let _ = tokio::time::timeout(Duration::from_secs(2), server.handle).await;
}
