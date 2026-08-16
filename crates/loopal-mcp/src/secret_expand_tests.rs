use std::collections::HashMap;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_ipc::IpcBudget;

use super::resolved_config::ResolvedMcpConfig;
use super::secret_expand::expand_mcp_config;
use super::secret_expand_test_support::{FakeClient, client, provenance, stdio};

#[tokio::test]
async fn expands_only_stdio_env_values() {
    let fake = FakeClient::success();
    let config = stdio(
        "runner",
        vec!["--safe"],
        HashMap::from([("TOKEN".into(), "Bearer {{secret:token}}".into())]),
    );
    let expanded = expand_mcp_config(
        config,
        Some(&client(fake.clone())),
        &provenance(),
        IpcBudget::Forbidden,
    )
    .await
    .unwrap();
    let ResolvedMcpConfig::Stdio { env, .. } = expanded else {
        unreachable!()
    };
    assert_eq!(&*env["TOKEN"], "Bearer exact-plaintext");
    assert_eq!(fake.calls(), 1);
}

#[tokio::test]
async fn expands_only_http_header_values() {
    let fake = FakeClient::success();
    let config = McpServerConfig::StreamableHttp {
        url: "https://example.test/mcp".into(),
        headers: HashMap::from([("Authorization".into(), "Bearer {{secret:token}}".into())]),
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
    };
    let expanded = expand_mcp_config(
        config,
        Some(&client(fake)),
        &provenance(),
        IpcBudget::Forbidden,
    )
    .await
    .unwrap();
    let ResolvedMcpConfig::StreamableHttp { headers, .. } = expanded else {
        unreachable!()
    };
    assert_eq!(&*headers["Authorization"], "Bearer exact-plaintext");
}
