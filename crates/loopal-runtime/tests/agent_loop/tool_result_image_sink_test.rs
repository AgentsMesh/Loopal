use base64::Engine;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_api::PermissionMode;
use loopal_tool_invocation::ToolImageBlock;
use serde_json::json;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

fn minimal_png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&32u32.to_be_bytes());
    bytes.extend_from_slice(&32u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[tokio::test]
async fn validated_image_flows_through_final_event_and_block_sink() {
    let (mut runner, mut events, _, _, _) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sink.png");
    std::fs::write(&path, minimal_png()).unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        temp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "image-sink-test",
    );
    let mut turn_ctx = make_turn_ctx();

    let stats = in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![(
            "read-image".into(),
            "ReadImage".into(),
            json!({"file_path": path}),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(stats.errors, 0);
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event.payload,
            AgentEventPayload::ToolResult {
                is_error: false,
                ..
            }
        ))
    );
    let ContentBlock::ToolResult { images, .. } = &runner.turns.view().messages()[0].content[0]
    else {
        panic!("expected ToolResult");
    };
    assert_eq!(images.len(), 1);
    match &images[0] {
        ToolImageBlock::Inline { data, .. } => {
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
                minimal_png()
            );
        }
        ToolImageBlock::SessionResource { byte_size, .. } => {
            assert_eq!(*byte_size, minimal_png().len());
        }
    }
}
