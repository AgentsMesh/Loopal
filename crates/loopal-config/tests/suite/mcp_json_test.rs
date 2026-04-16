use std::io::Write;

use loopal_config::McpServerConfig;
use loopal_config::mcp_json::load_mcp_json;
use tempfile::NamedTempFile;

fn write_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

#[test]
fn stdio_implicit_type() {
    let f =
        write_temp(r#"{ "mcpServers": { "gh": { "command": "npx", "args": ["-y", "server"] } } }"#);
    let result = load_mcp_json(f.path());
    assert_eq!(result.len(), 1);
    match &result["gh"] {
        McpServerConfig::Stdio {
            command,
            args,
            enabled,
            ..
        } => {
            assert_eq!(command, "npx");
            assert_eq!(args, &["-y", "server"]);
            assert!(*enabled);
        }
        _ => panic!("expected Stdio variant"),
    }
}

#[test]
fn streamable_http_explicit_type() {
    let f = write_temp(
        r#"{ "mcpServers": { "remote": { "type": "streamable-http", "url": "https://mcp.example.com", "headers": { "Authorization": "Bearer tok" } } } }"#,
    );
    let result = load_mcp_json(f.path());
    assert_eq!(result.len(), 1);
    match &result["remote"] {
        McpServerConfig::StreamableHttp {
            url,
            headers,
            enabled,
            timeout_ms,
            ..
        } => {
            assert_eq!(url, "https://mcp.example.com");
            assert_eq!(headers.get("Authorization").unwrap(), "Bearer tok");
            assert!(*enabled);
            assert_eq!(*timeout_ms, 30_000);
        }
        _ => panic!("expected StreamableHttp variant"),
    }
}

#[test]
fn custom_enabled_and_timeout() {
    let f = write_temp(
        r#"{ "mcpServers": { "slow": { "command": "slow-server", "enabled": false, "timeout_ms": 60000 } } }"#,
    );
    let result = load_mcp_json(f.path());
    match &result["slow"] {
        McpServerConfig::Stdio {
            enabled,
            timeout_ms,
            ..
        } => {
            assert!(!*enabled);
            assert_eq!(*timeout_ms, 60_000);
        }
        _ => panic!("expected Stdio variant"),
    }
}

#[test]
fn missing_command_skipped() {
    let f = write_temp(r#"{ "mcpServers": { "bad": { "args": ["--help"] } } }"#);
    let result = load_mcp_json(f.path());
    assert!(result.is_empty());
}

#[test]
fn missing_url_skipped() {
    let f = write_temp(r#"{ "mcpServers": { "bad": { "type": "streamable-http" } } }"#);
    let result = load_mcp_json(f.path());
    assert!(result.is_empty());
}

#[test]
fn unknown_type_skipped() {
    let f = write_temp(r#"{ "mcpServers": { "x": { "type": "grpc", "command": "cmd" } } }"#);
    let result = load_mcp_json(f.path());
    assert!(result.is_empty());
}

#[test]
fn missing_file_returns_empty() {
    let result = load_mcp_json(std::path::Path::new("/nonexistent/.mcp.json"));
    assert!(result.is_empty());
}

#[test]
fn invalid_json_returns_empty() {
    let f = write_temp("not json");
    let result = load_mcp_json(f.path());
    assert!(result.is_empty());
}

#[test]
fn empty_servers_returns_empty() {
    let f = write_temp(r#"{ "mcpServers": {} }"#);
    let result = load_mcp_json(f.path());
    assert!(result.is_empty());
}

#[test]
fn multiple_servers_parsed() {
    let f = write_temp(
        r#"{
        "mcpServers": {
            "a": { "command": "cmd-a" },
            "b": { "type": "streamable-http", "url": "https://b.com" }
        }
    }"#,
    );
    let result = load_mcp_json(f.path());
    assert_eq!(result.len(), 2);
    assert!(matches!(result["a"], McpServerConfig::Stdio { .. }));
    assert!(matches!(
        result["b"],
        McpServerConfig::StreamableHttp { .. }
    ));
}

#[test]
fn env_field_parsed() {
    let f =
        write_temp(r#"{ "mcpServers": { "s": { "command": "srv", "env": { "TOKEN": "abc" } } } }"#);
    let result = load_mcp_json(f.path());
    match &result["s"] {
        McpServerConfig::Stdio { env, .. } => {
            assert_eq!(env.get("TOKEN").unwrap(), "abc");
        }
        _ => panic!("expected Stdio"),
    }
}

// --- Integration: .mcp.json overrides settings.json within a single layer ---

#[test]
fn layer_mcp_json_overrides_and_adds() {
    use loopal_config::layer::LayerSource;
    use loopal_config::loader::load_layer_from_dir;

    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    std::fs::write(
        dir.join("settings.json"),
        r#"{ "mcp_servers": {
            "shared": { "type": "stdio", "command": "old-cmd" },
            "only_settings": { "type": "stdio", "command": "s" }
        } }"#,
    )
    .unwrap();

    std::fs::write(
        dir.join(".mcp.json"),
        r#"{ "mcpServers": {
            "shared": { "command": "new-cmd" },
            "only_mcp_json": { "command": "m" }
        } }"#,
    )
    .unwrap();

    let layer = load_layer_from_dir(dir, LayerSource::Project, None).unwrap();
    assert_eq!(layer.mcp_servers.len(), 3);
    // .mcp.json overrides same-name server from settings.json
    match &layer.mcp_servers["shared"] {
        McpServerConfig::Stdio { command, .. } => assert_eq!(command, "new-cmd"),
        _ => panic!("expected Stdio"),
    }
    // Both unique servers are present
    assert!(layer.mcp_servers.contains_key("only_settings"));
    assert!(layer.mcp_servers.contains_key("only_mcp_json"));
}
