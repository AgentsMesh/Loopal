use loopal_kernel::Kernel;
use loopal_types::config::Settings;
use loopal_types::hook::HookEvent;

fn make_kernel() -> Kernel {
    // Kernel::new with default settings works without any API keys;
    // providers simply won't be registered. This is sufficient for
    // testing accessor methods and basic construction.
    Kernel::new(Settings::default()).expect("Kernel::new with defaults should succeed")
}

#[test]
fn test_kernel_new_succeeds_with_defaults() {
    let _kernel = make_kernel();
}

#[test]
fn test_tool_registry_accessor() {
    let kernel = make_kernel();
    // Builtin tools should have been registered
    let _registry = kernel.tool_registry();
    let defs = kernel.tool_definitions();
    // At minimum, some builtin tools should exist (bash, read, write, etc.)
    assert!(
        !defs.is_empty(),
        "tool_definitions should not be empty after builtin registration"
    );
}

#[test]
fn test_provider_registry_accessor() {
    let kernel = make_kernel();
    let _registry = kernel.provider_registry();
    // Without API keys, no providers are registered, but the accessor should work
}

#[test]
fn test_hook_registry_accessor() {
    let kernel = make_kernel();
    let hooks = kernel.get_hooks(HookEvent::PreToolUse, None);
    // Default settings have no hooks
    assert!(hooks.is_empty());
}

#[test]
fn test_mcp_manager_accessor() {
    let kernel = make_kernel();
    let _mcp = kernel.mcp_manager();
}

#[test]
fn test_settings_accessor() {
    let kernel = make_kernel();
    let settings = kernel.settings();
    assert_eq!(settings.model, "claude-sonnet-4-20250514");
    assert_eq!(settings.max_turns, 50);
}

#[test]
fn test_get_tool_returns_none_for_unknown() {
    let kernel = make_kernel();
    assert!(kernel.get_tool("nonexistent_tool_xyz").is_none());
}

#[test]
fn test_get_tool_returns_some_for_builtin() {
    let kernel = make_kernel();
    // Builtin tools use capitalized names: "Bash", "Read", "Write", etc.
    let tool = kernel.get_tool("Bash");
    assert!(tool.is_some(), "Bash should be a registered builtin tool");
}

#[test]
fn test_tool_definitions_not_empty() {
    let kernel = make_kernel();
    let defs = kernel.tool_definitions();
    assert!(
        !defs.is_empty(),
        "builtins should produce non-empty tool definitions"
    );
    // Verify each definition has a non-empty name
    for def in &defs {
        assert!(!def.name.is_empty(), "tool definition name should not be empty");
        assert!(
            !def.description.is_empty(),
            "tool definition description should not be empty"
        );
    }
}

#[test]
fn test_kernel_shutdown_does_not_panic() {
    let mut kernel = make_kernel();
    kernel.shutdown(); // should not panic
}

#[tokio::test]
async fn test_start_mcp_no_servers() {
    let mut kernel = make_kernel();
    // With default settings (no MCP servers), start_mcp should be a no-op
    kernel
        .start_mcp()
        .await
        .expect("start_mcp with no servers should succeed");
}

#[test]
fn test_resolve_api_key_direct() {
    // Direct key takes precedence
    let result =
        loopal_kernel::resolve_api_key(&Some("my-direct-key".to_string()), &None);
    assert_eq!(result, Some("my-direct-key".to_string()));
}

#[test]
fn test_resolve_api_key_env_var() {
    let env_var_name = "TEST_RESOLVE_API_KEY_ENV_VAR_KERNEL";
    unsafe {
        std::env::set_var(env_var_name, "env-key-value");
    }

    let result =
        loopal_kernel::resolve_api_key(&None, &Some(env_var_name.to_string()));
    assert_eq!(result, Some("env-key-value".to_string()));

    unsafe {
        std::env::remove_var(env_var_name);
    }
}

#[test]
fn test_resolve_api_key_direct_over_env() {
    // Direct key should take precedence over env var
    let env_var_name = "TEST_RESOLVE_API_KEY_DIRECT_OVER_ENV";
    unsafe {
        std::env::set_var(env_var_name, "env-value");
    }

    let result = loopal_kernel::resolve_api_key(
        &Some("direct-value".to_string()),
        &Some(env_var_name.to_string()),
    );
    assert_eq!(result, Some("direct-value".to_string()));

    unsafe {
        std::env::remove_var(env_var_name);
    }
}

#[test]
fn test_resolve_api_key_empty() {
    // Empty direct key should not be returned
    let result = loopal_kernel::resolve_api_key(&Some(String::new()), &None);
    assert_eq!(result, None);

    // Both None
    let result = loopal_kernel::resolve_api_key(&None, &None);
    assert_eq!(result, None);

    // Empty env var should not be returned either
    let env_var_name = "TEST_RESOLVE_API_KEY_EMPTY_ENV";
    unsafe {
        std::env::set_var(env_var_name, "");
    }
    let result =
        loopal_kernel::resolve_api_key(&None, &Some(env_var_name.to_string()));
    assert_eq!(result, None);
    unsafe {
        std::env::remove_var(env_var_name);
    }

    // Nonexistent env var
    let result = loopal_kernel::resolve_api_key(
        &None,
        &Some("NONEXISTENT_VAR_THAT_DOES_NOT_EXIST".to_string()),
    );
    assert_eq!(result, None);
}
