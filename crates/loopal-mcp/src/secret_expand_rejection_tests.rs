use std::collections::HashMap;

use loopal_config::{CwdIsolation, McpServerConfig, McpSharing};
use loopal_ipc::IpcBudget;

use super::secret_expand::{CONFIG_SECRET_ERROR, expand_mcp_config};
use super::secret_expand_test_support::{FakeClient, client, provenance, stdio};

#[tokio::test]
async fn rejects_ineligible_fields_before_secret_fetch() {
    let fake = FakeClient::success();
    let cases = [
        stdio("{{secret:token}}", vec![], HashMap::new()),
        stdio("runner", vec!["{{secret:token}}"], HashMap::new()),
        stdio(
            "runner",
            vec![],
            HashMap::from([("TOKEN".into(), "<secret_ref:token>".into())]),
        ),
        McpServerConfig::StreamableHttp {
            url: "https://{{secret:token}}.test".into(),
            headers: HashMap::new(),
            enabled: true,
            timeout_ms: 100,
            sharing: McpSharing::HubSingleton,
        },
        McpServerConfig::Stdio {
            command: "runner".into(),
            args: Vec::new(),
            env: HashMap::new(),
            enabled: true,
            timeout_ms: 100,
            sharing: McpSharing::HubSingleton,
            cwd_isolation: Some(CwdIsolation {
                arg: "{{secret:token}}".into(),
                cache_subdir: None,
            }),
        },
        stdio(
            "runner",
            vec![],
            HashMap::from([("{{secret:token}}".into(), "ordinary".into())]),
        ),
        McpServerConfig::Stdio {
            command: "runner".into(),
            args: Vec::new(),
            env: HashMap::new(),
            enabled: true,
            timeout_ms: 100,
            sharing: McpSharing::HubSingleton,
            cwd_isolation: Some(CwdIsolation {
                arg: "ordinary".into(),
                cache_subdir: Some("{{secret:token}}".into()),
            }),
        },
    ];
    for config in cases {
        let error = match expand_mcp_config(
            config,
            Some(&client(fake.clone())),
            &provenance(),
            IpcBudget::Forbidden,
        )
        .await
        {
            Ok(_) => panic!("ineligible config accepted"),
            Err(error) => error,
        };
        assert_eq!(error, CONFIG_SECRET_ERROR);
    }
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn missing_client_or_fetch_failure_is_safe_and_fallible() {
    let config = stdio(
        "runner",
        vec![],
        HashMap::from([("TOKEN".into(), "{{secret:private_name}}".into())]),
    );
    let missing =
        match expand_mcp_config(config.clone(), None, &provenance(), IpcBudget::Forbidden).await {
            Ok(_) => panic!("missing secret client accepted"),
            Err(error) => error,
        };
    assert_eq!(missing, CONFIG_SECRET_ERROR);
    let fake = FakeClient::failing("audit/private_name/plaintext");
    let error = match expand_mcp_config(
        config,
        Some(&client(fake)),
        &provenance(),
        IpcBudget::Forbidden,
    )
    .await
    {
        Ok(_) => panic!("failed secret fetch accepted"),
        Err(error) => error,
    };
    assert_eq!(error, CONFIG_SECRET_ERROR);
    assert!(!error.contains("private_name"));
    assert!(!error.contains("plaintext"));
}
