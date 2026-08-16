use std::collections::HashMap;

use indexmap::IndexMap;
use loopal_config::{McpServerConfig, McpSharing};

use super::factory::{build_local_provider, canonical_or_self, load_servers_by_sharing};

#[test]
fn load_servers_filters_sharing_and_falls_back_on_bad_config() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".loopal")).unwrap();
    std::fs::write(
        temp.path().join(".loopal/settings.json"),
        r#"{
            "mcp_servers": {
                "hub": {"type":"stdio","command":"/usr/bin/true","sharing":"hub-singleton"},
                "agent": {"type":"stdio","command":"/usr/bin/true","sharing":"per-agent"}
            }
        }"#,
    )
    .unwrap();
    let hub = load_servers_by_sharing(temp.path(), McpSharing::HubSingleton);
    assert_eq!(hub.keys().collect::<Vec<_>>(), ["hub"]);

    std::fs::write(temp.path().join(".loopal/settings.json"), "{").unwrap();
    assert!(load_servers_by_sharing(temp.path(), McpSharing::HubSingleton).is_empty());
}

#[test]
fn canonical_or_self_canonicalizes_existing_and_preserves_missing() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(
        canonical_or_self(temp.path()),
        temp.path().canonicalize().unwrap()
    );
    let missing = temp.path().join("missing");
    assert_eq!(canonical_or_self(&missing), missing);
}

#[tokio::test]
async fn build_provider_handles_empty_plain_and_vault_backed_configs() {
    let empty = build_local_provider(None, std::path::Path::new("."), IndexMap::new()).await;
    assert!(
        empty
            .manager()
            .read()
            .await
            .prepare_connections(&IndexMap::new())
            .await
            .is_empty()
    );

    let plain_config = McpServerConfig::Stdio {
        command: "/usr/bin/true".into(),
        args: Vec::new(),
        env: HashMap::from([("TOKEN".into(), "{{secret:missing}}".into())]),
        enabled: true,
        timeout_ms: 10,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    };
    let plain = build_local_provider(
        None,
        std::path::Path::new("."),
        IndexMap::from([("plain".into(), plain_config.clone())]),
    )
    .await;
    let mut plain_prepared = plain
        .manager()
        .read()
        .await
        .prepare_connections(&IndexMap::from([("plain".into(), plain_config)]))
        .await;
    assert_eq!(plain_prepared.len(), 1);
    plain_prepared[0].connect().await;
    assert!(plain_prepared[0].status.is_failed());
    assert_eq!(
        plain_prepared[0].errors,
        ["MCP server secret configuration unavailable"]
    );

    let (temp, vault) = super::test_vault::service(&[("token", "secret-value")]).await;
    let config = McpServerConfig::Stdio {
        command: "/usr/bin/true".into(),
        args: Vec::new(),
        env: HashMap::from([("TOKEN".into(), "{{secret:token}}".into())]),
        enabled: true,
        timeout_ms: 10,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    };
    let provider = build_local_provider(
        Some(&vault),
        temp.path(),
        IndexMap::from([("vault".into(), config.clone())]),
    )
    .await;
    let mut prepared = provider
        .manager()
        .read()
        .await
        .prepare_connections(&IndexMap::from([("vault".into(), config)]))
        .await;
    assert_eq!(prepared.len(), 1);
    prepared[0].connect().await;
    assert_ne!(
        prepared[0].errors,
        ["MCP server secret configuration unavailable"]
    );
}
