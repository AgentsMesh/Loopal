use std::sync::Arc;

use chrono::Utc;
use loopal_config::Settings;
use loopal_context::ContextPipeline;
use loopal_error::TerminateReason;
use loopal_kernel::Kernel;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_provider_api::{StopReason, StreamChunk};
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend, agent_loop};
use loopal_storage::Session;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

use super::mock_provider::make_runner_with_mock_provider;

fn make_session(id: &str) -> Session {
    Session {
        id: id.to_string(),
        title: "".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    }
}

#[tokio::test]
async fn test_agent_loop_immediate_channel_close() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);

    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let tmp = std::env::temp_dir().join(format!("la_loop_{}", std::process::id()));
    let params = AgentLoopParams {
        kernel,
        session: make_session("test-loop"),
        messages: Vec::new(),
        model: "claude-sonnet-4-20250514".to_string(),
        compact_model: None,
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Bypass,
        max_turns: 10,
        frontend,
        session_manager: SessionManager::with_base_dir(tmp),
        context_pipeline: ContextPipeline::new(),
        tool_filter: None,
        shared: None,
        interactive: true,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
        memory_channel: None,
    };

    // Drop senders to close channels
    drop(mbox_tx);
    drop(ctrl_tx);

    let result = agent_loop(params).await;
    assert!(result.is_ok());

    let mut events = Vec::new();
    while let Ok(e) = event_rx.try_recv() {
        events.push(e);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e.payload, AgentEventPayload::Started))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e.payload, AgentEventPayload::Finished))
    );
}

#[tokio::test]
async fn test_agent_loop_max_turns_reached() {
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);

    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let tmp = std::env::temp_dir().join(format!("la_turns_{}", std::process::id()));
    let params = AgentLoopParams {
        kernel,
        session: make_session("test-turns"),
        messages: vec![Message::user("hello")],
        model: "claude-sonnet-4-20250514".to_string(),
        compact_model: None,
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Bypass,
        max_turns: 0,
        frontend,
        session_manager: SessionManager::with_base_dir(tmp),
        context_pipeline: ContextPipeline::new(),
        tool_filter: None,
        shared: None,
        interactive: true,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
        memory_channel: None,
    };

    let result = agent_loop(params).await;
    let output = result.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::MaxTurns);

    let mut events = Vec::new();
    while let Ok(e) = event_rx.try_recv() {
        events.push(e);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e.payload, AgentEventPayload::MaxTurnsReached { .. }))
    );
}

#[tokio::test]
async fn test_full_run_text_only_then_input_close() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "Hi there!".to_string(),
        }),
        Ok(StreamChunk::Usage {
            input_tokens: 5,
            output_tokens: 3,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            thinking_tokens: 0,
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ];
    let (mut runner, mut event_rx, mbox_tx, ctrl_tx) = make_runner_with_mock_provider(chunks);

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if matches!(event.payload, AgentEventPayload::AwaitingInput) {
                drop(mbox_tx);
                drop(ctrl_tx);
                while event_rx.recv().await.is_some() {}
                break;
            }
        }
    });

    let result = runner.run().await;
    assert!(result.is_ok());
    assert!(runner.params.messages.len() >= 2);
}

#[tokio::test]
async fn test_full_run_with_tool_execution() {
    let tmp_file = std::env::temp_dir().join(format!("la_run_test_{}.txt", std::process::id()));
    std::fs::write(&tmp_file, "test content").unwrap();

    // Two LLM calls: first returns Read tool, second ends the turn with text.
    let calls = vec![
        vec![
            Ok(StreamChunk::ToolUse {
                id: "tc-1".to_string(),
                name: "Read".to_string(),
                input: serde_json::json!({"file_path": tmp_file.to_str().unwrap()}),
            }),
            Ok(StreamChunk::Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                thinking_tokens: 0,
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "Done.".to_string(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = super::mock_provider::make_multi_runner(calls);
    runner.params.max_turns = 5;

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let result = runner.run().await;
    assert!(result.is_ok());
    // user + assistant(tool_use) + user(tool_result) + assistant(text)
    assert!(runner.params.messages.len() >= 3);

    let _ = std::fs::remove_file(&tmp_file);
}
