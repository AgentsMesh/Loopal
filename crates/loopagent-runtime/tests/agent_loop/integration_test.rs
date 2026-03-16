use std::sync::Arc;

use chrono::Utc;
use loopagent_context::ContextPipeline;
use loopagent_kernel::Kernel;
use loopagent_runtime::{agent_loop, AgentLoopParams, AgentMode, SessionManager};
use loopagent_storage::Session;
use loopagent_types::config::Settings;
use loopagent_types::event::AgentEvent;
use loopagent_types::message::Message;
use loopagent_types::permission::PermissionMode;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_agent_loop_immediate_channel_close() {
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let (_input_tx, input_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel(16);

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = Session {
        id: "test-loop".to_string(),
        title: "".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_loop_{}",
        std::process::id()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        messages: Vec::new(),
        model: "claude-sonnet-4-20250514".to_string(),
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::BypassPermissions,
        max_turns: 10,
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline: ContextPipeline::new(),
    };

    // Drop input sender immediately so channel closes
    drop(_input_tx);

    // agent_loop should exit gracefully
    let result = agent_loop(params).await;
    assert!(result.is_ok());

    // Should have received Started, AwaitingInput, Finished
    let mut events = Vec::new();
    while let Ok(e) = event_rx.try_recv() {
        events.push(e);
    }
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Started)));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Finished)));
}

#[tokio::test]
async fn test_agent_loop_max_turns_reached() {
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let (_input_tx, input_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel(16);

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = Session {
        id: "test-turns".to_string(),
        title: "".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_turns_{}",
        std::process::id()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        // Pre-fill a message so we don't block on input
        messages: vec![Message::user("hello")],
        model: "claude-sonnet-4-20250514".to_string(),
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::BypassPermissions,
        max_turns: 0, // Immediate max turns
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline: ContextPipeline::new(),
    };

    let result = agent_loop(params).await;
    assert!(result.is_ok());

    let mut events = Vec::new();
    while let Ok(e) = event_rx.try_recv() {
        events.push(e);
    }
    assert!(events.iter().any(|e| matches!(e, AgentEvent::MaxTurnsReached { .. })));
}

#[tokio::test]
async fn test_full_run_text_only_then_input_close() {
    let chunks = vec![
        Ok(StreamChunk::Text { text: "Hi there!".to_string() }),
        Ok(StreamChunk::Usage { input_tokens: 5, output_tokens: 3 }),
        Ok(StreamChunk::Done),
    ];
    let (mut runner, mut event_rx, input_tx) = make_runner_with_mock_provider(chunks);

    // After LLM responds without tool use, it waits for input.
    // Close the channel to end the loop.
    tokio::spawn(async move {
        // Wait until AwaitingInput is received after the LLM response
        loop {
            if let Some(event) = event_rx.recv().await {
                if matches!(event, AgentEvent::AwaitingInput) {
                    // Close channel to end loop
                    drop(input_tx);
                    // Drain remaining events
                    while event_rx.recv().await.is_some() {}
                    break;
                }
            } else {
                break;
            }
        }
    });

    let result = runner.run().await;
    assert!(result.is_ok());
    // Should have the user message + assistant response in messages
    assert!(runner.params.messages.len() >= 2);
}

#[tokio::test]
async fn test_full_run_with_tool_execution() {
    // Create a temp file for Read tool to find
    let tmp_file = std::env::temp_dir().join(format!(
        "loopagent_run_test_{}.txt", std::process::id()
    ));
    std::fs::write(&tmp_file, "test content").unwrap();

    let chunks = vec![
        Ok(StreamChunk::ToolUse {
            id: "tc-1".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({"file_path": tmp_file.to_str().unwrap()}),
        }),
        Ok(StreamChunk::Usage { input_tokens: 10, output_tokens: 5 }),
        Ok(StreamChunk::Done),
    ];
    let (mut runner, mut event_rx, input_tx) = make_runner_with_mock_provider(chunks);

    // After tool execution, it will try to call LLM again (but mock provider
    // has no more chunks). The second stream_llm call will get an error since
    // the mock has been consumed. The run loop will then error or reach max turns.
    // Let's set max_turns to 1 so it stops after tool execution.
    runner.params.max_turns = 1;

    tokio::spawn(async move {
        while event_rx.recv().await.is_some() {}
        drop(input_tx);
    });

    let result = runner.run().await;
    // Should succeed (max turns reached after tool execution)
    assert!(result.is_ok());

    // Should have: user msg, assistant (tool use), tool result
    assert!(runner.params.messages.len() >= 3);

    let _ = std::fs::remove_file(&tmp_file);
}
