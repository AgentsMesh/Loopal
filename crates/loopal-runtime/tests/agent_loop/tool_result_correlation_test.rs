use loopal_protocol::AgentEventPayload;
use loopal_tool_api::PermissionMode;
use serde_json::json;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

#[tokio::test]
async fn finalized_batch_results_retain_batch_correlation() {
    let (mut runner, mut events, _, _, _) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;
    let temp = tempfile::tempdir().unwrap();
    let mut tools = Vec::new();
    for index in 0..3 {
        let path = temp.path().join(format!("input-{index}.txt"));
        std::fs::write(&path, format!("value-{index}")).unwrap();
        tools.push((
            format!("read-{index}"),
            "Read".to_string(),
            json!({"file_path": path}),
        ));
    }
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        temp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "correlation-test",
    );
    let mut turn_ctx = make_turn_ctx();

    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tools,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let mut batch_correlation = None;
    let mut result_correlations = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event.payload {
            AgentEventPayload::ToolBatchStart { .. } => {
                batch_correlation = Some(event.correlation_id);
                assert_eq!(event.turn_id, 1);
            }
            AgentEventPayload::ToolResult { .. } => {
                result_correlations.push(event.correlation_id);
                assert_eq!(event.turn_id, 1);
            }
            _ => {}
        }
    }
    let batch_correlation = batch_correlation.expect("missing ToolBatchStart");
    assert_ne!(batch_correlation, 0);
    assert_eq!(result_correlations.len(), 3);
    assert!(
        result_correlations
            .iter()
            .all(|correlation| *correlation == batch_correlation)
    );
}
