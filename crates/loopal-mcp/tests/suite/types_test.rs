//! Unit tests for types, reconnect policy, and OAuth callback parsing.

use std::time::Duration;

use loopal_config::McpServerConfig;
use loopal_mcp::reconnect::ReconnectPolicy;
use loopal_mcp::types::ConnectionStatus;

// --- ConnectionStatus ---

#[test]
fn test_status_disconnected() {
    let s = ConnectionStatus::Disconnected;
    assert!(!s.is_connected());
    assert!(!s.is_failed());
    assert_eq!(s.to_string(), "disconnected");
}

#[test]
fn test_status_connecting() {
    let s = ConnectionStatus::Connecting;
    assert!(!s.is_connected());
    assert!(!s.is_failed());
    assert_eq!(s.to_string(), "connecting");
}

#[test]
fn test_status_connected() {
    let s = ConnectionStatus::Connected;
    assert!(s.is_connected());
    assert!(!s.is_failed());
    assert_eq!(s.to_string(), "connected");
}

#[test]
fn test_status_failed() {
    let s = ConnectionStatus::Failed("timeout".into());
    assert!(!s.is_connected());
    assert!(s.is_failed());
    assert_eq!(s.to_string(), "failed: timeout");
}

// --- ReconnectPolicy ---

#[test]
fn test_reconnect_default_values() {
    let policy = ReconnectPolicy::default();
    assert_eq!(policy.max_attempts, 6);
    assert_eq!(policy.base_delay, Duration::from_secs(2));
    assert_eq!(policy.backoff_factor, 2.0);
}

#[test]
fn test_reconnect_delay_calculation() {
    let policy = ReconnectPolicy::default();
    assert_eq!(policy.delay_for_attempt(0), Duration::from_secs(2));
    assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(4));
    assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(8));
    assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(16));
    assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(32));
    assert_eq!(policy.delay_for_attempt(5), Duration::from_secs(64));
}

#[test]
fn test_is_reconnectable_stdio() {
    let config = McpServerConfig::Stdio {
        command: "test".into(),
        args: vec![],
        env: Default::default(),
        enabled: true,
        timeout_ms: 5000,
    };
    assert!(!ReconnectPolicy::is_reconnectable(&config));
}

#[test]
fn test_is_reconnectable_http() {
    let config = McpServerConfig::StreamableHttp {
        url: "https://example.com".into(),
        headers: Default::default(),
        enabled: true,
        timeout_ms: 5000,
    };
    assert!(ReconnectPolicy::is_reconnectable(&config));
}

// --- McpServerConfig serde ---

#[test]
fn test_stdio_config_serde_roundtrip() {
    let json = r#"{"type": "stdio", "command": "npx", "args": ["-y", "mcp"]}"#;
    let config: McpServerConfig = serde_json::from_str(json).expect("parse failed");
    let McpServerConfig::Stdio { command, args, .. } = &config else {
        panic!("expected Stdio");
    };
    assert_eq!(command, "npx");
    assert_eq!(args, &["-y", "mcp"]);
    assert!(config.enabled());
    assert_eq!(config.timeout_ms(), 30_000);
}

#[test]
fn test_http_config_serde_roundtrip() {
    let json = r#"{"type": "streamable-http", "url": "https://mcp.example.com/v1"}"#;
    let config: McpServerConfig = serde_json::from_str(json).expect("parse failed");
    let McpServerConfig::StreamableHttp { url, .. } = &config else {
        panic!("expected StreamableHttp");
    };
    assert_eq!(url, "https://mcp.example.com/v1");
}

#[test]
fn test_http_config_with_headers() {
    let json = r#"{
        "type": "streamable-http",
        "url": "https://mcp.example.com",
        "headers": {"Authorization": "Bearer tok123"}
    }"#;
    let config: McpServerConfig = serde_json::from_str(json).expect("parse failed");
    let McpServerConfig::StreamableHttp { headers, .. } = &config else {
        panic!("expected StreamableHttp");
    };
    assert_eq!(headers.get("Authorization").unwrap(), "Bearer tok123");
}

#[test]
fn test_config_disabled() {
    let json = r#"{"type": "stdio", "command": "x", "enabled": false}"#;
    let config: McpServerConfig = serde_json::from_str(json).unwrap();
    assert!(!config.enabled());
}

#[test]
fn test_invalid_type_fails() {
    let json = r#"{"type": "websocket", "url": "ws://localhost"}"#;
    let result = serde_json::from_str::<McpServerConfig>(json);
    assert!(result.is_err());
}

#[test]
fn test_missing_type_fails() {
    let json = r#"{"command": "npx"}"#;
    let result = serde_json::from_str::<McpServerConfig>(json);
    assert!(result.is_err());
}
