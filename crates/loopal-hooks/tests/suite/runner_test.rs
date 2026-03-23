use loopal_config::HookConfig;
use loopal_hooks::run_hook;

fn make_hook(command: &str, timeout_ms: u64) -> HookConfig {
    HookConfig {
        event: loopal_config::HookEvent::PreToolUse,
        command: command.to_string(),
        tool_filter: None,
        timeout_ms,
    }
}

#[tokio::test]
async fn test_run_hook_echo_success() {
    let hook = make_hook("echo hello", 5000);
    let data = serde_json::json!({"key": "value"});
    let result = run_hook(&hook, data).await.expect("hook should succeed");

    assert_eq!(result.exit_code, 0);
    assert!(result.is_success());
    assert!(
        result.stdout.trim().contains("hello"),
        "stdout should contain 'hello', got: {}",
        result.stdout
    );
    assert!(result.stderr.is_empty() || result.stderr.trim().is_empty());
}

#[tokio::test]
async fn test_run_hook_failing_command() {
    let hook = make_hook("exit 42", 5000);
    let data = serde_json::json!({});
    let result = run_hook(&hook, data)
        .await
        .expect("hook should not error on non-zero exit");

    assert_eq!(result.exit_code, 42);
    assert!(!result.is_success());
}

#[tokio::test]
async fn test_run_hook_receives_stdin_data() {
    // The hook reads stdin and writes it to stdout via cat
    let hook = make_hook("cat", 5000);
    let data = serde_json::json!({"tool_name": "Bash", "some_field": 123});
    let result = run_hook(&hook, data.clone())
        .await
        .expect("hook should succeed");

    assert_eq!(result.exit_code, 0);
    // The stdout should contain the JSON we sent
    let parsed: serde_json::Value =
        serde_json::from_str(result.stdout.trim()).expect("stdout should be valid JSON");
    assert_eq!(parsed, data);
}

#[tokio::test]
async fn test_run_hook_stderr_captured() {
    let hook = make_hook("echo error_msg >&2", 5000);
    let data = serde_json::json!({});
    let result = run_hook(&hook, data).await.expect("hook should succeed");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stderr.contains("error_msg"),
        "stderr should contain 'error_msg', got: {}",
        result.stderr
    );
}

#[tokio::test]
async fn test_run_hook_timeout() {
    // Very short timeout with a long-running command
    let hook = make_hook("sleep 60", 100);
    let data = serde_json::json!({});
    let result = run_hook(&hook, data).await;

    assert!(result.is_err(), "hook should timeout");
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("timed out") || err_msg.contains("Timeout") || err_msg.contains("timeout"),
        "error should mention timeout, got: {err_msg}"
    );
}

#[tokio::test]
async fn test_run_hook_invalid_command() {
    // A command that doesn't exist — sh -c will still run, but the command fails
    let hook = make_hook("nonexistent_command_xyz_12345", 5000);
    let data = serde_json::json!({});
    let result = run_hook(&hook, data)
        .await
        .expect("hook should not error at execution level");

    // The exit code should be non-zero (command not found = 127 on most systems)
    assert_ne!(result.exit_code, 0);
    assert!(!result.is_success());
}

#[tokio::test]
async fn test_run_hook_with_complex_json_data() {
    // Test that complex nested JSON is correctly serialized and passed via stdin
    let hook = make_hook("cat", 5000);
    let data = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "echo hello",
            "timeout": 30000
        },
        "nested": {
            "array": [1, 2, 3, "four", null, true],
            "deep": {
                "key": "value",
                "number": 42.5
            }
        },
        "empty_object": {},
        "empty_array": [],
        "unicode": "\u{1f600} emoji and \u{4e2d}\u{6587}"
    });
    let result = run_hook(&hook, data.clone())
        .await
        .expect("hook should succeed with complex JSON");

    assert_eq!(result.exit_code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(result.stdout.trim()).expect("stdout should be valid JSON");
    assert_eq!(parsed, data, "round-tripped JSON should match");
}

#[tokio::test]
async fn test_run_hook_with_large_stdin_data() {
    // Test that large JSON payloads are handled correctly through stdin pipe
    let hook = make_hook("wc -c", 5000);
    let large_string = "x".repeat(100_000);
    let data = serde_json::json!({"payload": large_string});
    let result = run_hook(&hook, data.clone())
        .await
        .expect("hook should handle large stdin");

    assert_eq!(result.exit_code, 0);
    let byte_count: usize = result
        .stdout
        .trim()
        .parse()
        .expect("wc -c should output a number");
    // The serialized JSON will be larger than 100_000 bytes due to the key and quoting
    assert!(
        byte_count > 100_000,
        "byte count should reflect the large payload, got {byte_count}"
    );
}

#[tokio::test]
async fn test_run_hook_exit_code_preserved() {
    // Various exit codes should be preserved
    for code in [0, 1, 2, 42, 127] {
        let hook = make_hook(&format!("exit {code}"), 5000);
        let data = serde_json::json!({});
        let result = run_hook(&hook, data)
            .await
            .expect("hook should not error at execution level");
        assert_eq!(
            result.exit_code, code,
            "exit code {code} should be preserved"
        );
    }
}

#[tokio::test]
async fn test_run_hook_combined_stdout_stderr() {
    // Test that both stdout and stderr are captured independently
    let hook = make_hook("echo stdout_msg; echo stderr_msg >&2", 5000);
    let data = serde_json::json!({});
    let result = run_hook(&hook, data).await.expect("hook should succeed");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("stdout_msg"),
        "stdout should contain stdout_msg, got: {}",
        result.stdout
    );
    assert!(
        result.stderr.contains("stderr_msg"),
        "stderr should contain stderr_msg, got: {}",
        result.stderr
    );
}

#[tokio::test]
async fn test_run_hook_post_tool_use_event() {
    // Verify that hooks with PostToolUse event type also work correctly
    let hook = HookConfig {
        event: loopal_config::HookEvent::PostToolUse,
        command: "echo post-hook".to_string(),
        tool_filter: None,
        timeout_ms: 5000,
    };
    let data = serde_json::json!({"tool_name": "Read", "result": "ok"});
    let result = run_hook(&hook, data)
        .await
        .expect("post-hook should succeed");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("post-hook"));
}
