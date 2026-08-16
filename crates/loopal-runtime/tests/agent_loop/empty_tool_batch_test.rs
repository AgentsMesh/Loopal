use loopal_runtime::agent_loop::StreamingToolHandle;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

#[tokio::test]
async fn execute_tools_accepts_an_empty_batch_without_recording_results() {
    let (mut runner, _events, _mailbox, _control, _permission) = make_runner_with_channels();
    let mut turn = make_turn_ctx();

    let stats = in_turn(runner.execute_tools(&mut turn, vec![], StreamingToolHandle::empty()))
        .await
        .unwrap();

    assert_eq!((stats.approved, stats.denied, stats.errors), (0, 0, 0));
    assert!(runner.turns.view().is_empty());
}
