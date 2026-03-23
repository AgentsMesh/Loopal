//! Tests for LoopalError From conversions and sub-error Display impls.

use loopal_error::{
    ConfigError, HookError, LoopalError, McpError, ProviderError, StorageError, ToolError,
};

#[test]
fn test_loopal_error_display_mcp() {
    let err = LoopalError::Mcp(McpError::ServerNotFound("test-server".into()));
    assert_eq!(format!("{err}"), "MCP error: Server not found: test-server");
}

#[test]
fn test_loopal_error_display_other() {
    let err = LoopalError::Other("unexpected".into());
    assert_eq!(format!("{err}"), "unexpected");
}

// --- From conversions ---

#[test]
fn test_loopal_error_from_provider_error() {
    let provider_err = ProviderError::StreamEnded;
    let err: LoopalError = provider_err.into();
    assert!(matches!(err, LoopalError::Provider(_)));
}

#[test]
fn test_loopal_error_from_tool_error() {
    let tool_err = ToolError::Timeout(5000);
    let err: LoopalError = tool_err.into();
    assert!(matches!(err, LoopalError::Tool(_)));
}

#[test]
fn test_loopal_error_from_config_error() {
    let config_err = ConfigError::Parse("bad toml".into());
    let err: LoopalError = config_err.into();
    assert!(matches!(err, LoopalError::Config(_)));
}

#[test]
fn test_loopal_error_from_hook_error() {
    let hook_err = HookError::Timeout("slow hook".into());
    let err: LoopalError = hook_err.into();
    assert!(matches!(err, LoopalError::Hook(_)));
}

#[test]
fn test_loopal_error_from_mcp_error() {
    let mcp_err = McpError::ConnectionFailed("refused".into());
    let err: LoopalError = mcp_err.into();
    assert!(matches!(err, LoopalError::Mcp(_)));
}

// --- ToolError Display ---

#[test]
fn test_tool_error_display_not_found() {
    let err = ToolError::NotFound("bash".into());
    assert_eq!(format!("{err}"), "Tool not found: bash");
}

#[test]
fn test_tool_error_display_invalid_input() {
    let err = ToolError::InvalidInput("missing field".into());
    assert_eq!(format!("{err}"), "Invalid input: missing field");
}

#[test]
fn test_tool_error_display_execution_failed() {
    let err = ToolError::ExecutionFailed("segfault".into());
    assert_eq!(format!("{err}"), "Execution failed: segfault");
}

#[test]
fn test_tool_error_display_timeout() {
    let err = ToolError::Timeout(30000);
    assert_eq!(format!("{err}"), "Timeout after 30000ms");
}

// --- ConfigError Display ---

#[test]
fn test_config_error_display_invalid_value() {
    let err = ConfigError::InvalidValue {
        field: "max_turns".into(),
        reason: "must be positive".into(),
    };
    assert_eq!(
        format!("{err}"),
        "Invalid value for max_turns: must be positive"
    );
}

// --- StorageError Display ---

#[test]
fn test_storage_error_display_home_dir_not_found() {
    let err = StorageError::HomeDirNotFound;
    assert_eq!(format!("{err}"), "Could not determine home directory");
}

// --- HookError Display ---

#[test]
fn test_hook_error_display_execution_failed() {
    let err = HookError::ExecutionFailed("command not found".into());
    assert_eq!(format!("{err}"), "Hook execution failed: command not found");
}

#[test]
fn test_hook_error_display_timeout() {
    let err = HookError::Timeout("pre_tool_use".into());
    assert_eq!(format!("{err}"), "Hook timeout: pre_tool_use");
}

// --- McpError Display ---

#[test]
fn test_mcp_error_display_protocol() {
    let err = McpError::Protocol("invalid response".into());
    assert_eq!(format!("{err}"), "Protocol error: invalid response");
}
