use loopal_protocol::AgentStatus;

use super::support::{FAIL_AWAITING_INPUT, FAIL_ERROR, FAIL_FINISHED, make_fixture};
use crate::agent_loop::{InterruptHandle, LifecycleMode};

#[test]
fn lifecycle_mode_and_interrupt_defaults_cover_all_variants() {
    assert!(LifecycleMode::Persistent.is_persistent());
    assert!(!LifecycleMode::Persistent.is_one_shot());
    assert!(LifecycleMode::Ephemeral.is_one_shot());
    assert!(!LifecycleMode::Ephemeral.waits_for_workflow());
    assert!(LifecycleMode::WorkflowEphemeral.waits_for_workflow());

    let interrupt = InterruptHandle::default();
    assert!(!interrupt.signal.is_signaled());
}

#[tokio::test]
async fn lifecycle_shutdown_preserves_primary_error_when_error_events_fail() {
    let mut fixture = make_fixture();
    fixture
        .frontend
        .set_fail_mask(FAIL_AWAITING_INPUT | FAIL_ERROR | FAIL_FINISHED);
    drop(fixture.input_tx);

    let error = fixture.runner.run().await.unwrap_err();
    assert!(error.to_string().contains("injected event failure"));
    assert_ne!(fixture.runner.status, AgentStatus::Finished);
}

#[tokio::test]
async fn cleanup_preserves_the_log_of_a_live_background_task() {
    let fixture = make_fixture();
    let fifo = fixture.temp.path().join("background-input");
    let command = format!(
        "mkfifo '{}' && read -r value < '{}'",
        fifo.display(),
        fifo.display()
    );
    let bash = fixture.runner.params.deps.kernel.get_tool("Bash").unwrap();
    let started = bash
        .execute(
            serde_json::json!({
                "command": command,
                "run_in_background": true,
                "description": "runtime cleanup coverage",
            }),
            &fixture.runner.tool_ctx,
        )
        .await
        .unwrap();
    assert!(!started.is_error, "{}", started.content);
    let task_id = started
        .content
        .lines()
        .find_map(|line| line.strip_prefix("process_id: "))
        .unwrap();
    let store = fixture.runner.params.deps.kernel.bg_store();
    let log_path = store
        .read_task(task_id, |task| task.log_path().to_path_buf())
        .unwrap();

    fixture.runner.cleanup_session_tmp().await;
    assert!(log_path.exists());

    let stopped = loopal_tool_background::ops::bg_stop(store, task_id).await;
    assert!(!stopped.is_error, "{}", stopped.content);
    let _ = std::fs::remove_file(fifo);
}
