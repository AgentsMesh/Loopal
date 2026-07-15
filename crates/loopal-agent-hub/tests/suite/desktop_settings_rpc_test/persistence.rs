#[tokio::test]
async fn desktop_settings_rpc_persists_atomically_and_redacts_providers() {
    let root = tempfile::tempdir().unwrap();
    let project_dir = root.path().join(".loopal");
    let user_dir = root.path().join(".loopal-user");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::write(
        project_dir.join("settings.json"),
        serde_json::to_vec(&json!({
            "model": "project-override",
            "model_routing": {"summarization": "project-summary"}
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        user_dir.join("settings.json"),
        serde_json::to_vec(&json!({
            "model": "before",
            "model_routing": {"summarization": "user-summary"},
            "providers": {"anthropic": {
                "api_key": "existing-provider-value", "api_key_env": "ANTHROPIC_KEY",
                "base_url": "https://user:password@example.test/?token=value", "unknown": true
            }},
            "sandbox": {"network": {"denied_domains": ["blocked.test"]}},
            "unknown_top_level": {"preserve": true}
        }))
        .unwrap(),
    )
    .unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let before = conn
        .send_request(
            methods::DESKTOP_GET_SETTINGS.name,
            json!({"workspaceId": "local-workspace"}),
        )
        .await
        .unwrap();
    assert_eq!(before["settings"]["model"], "before");
    assert_eq!(
        before["settings"]["modelRouting"]["summarization"],
        "user-summary"
    );
    assert!(
        before["configuredProviders"]
            .as_array()
            .unwrap()
            .contains(&json!("anthropic"))
    );
    assert!(
        before["resolvedEntries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["key"] == "model" && entry["value"] == "project-override" })
    );
    assert_eq!(before["providers"]["anthropic"]["apiKeyConfigured"], true);
    assert_eq!(
        before["providers"]["anthropic"]["apiKeyEnv"],
        "ANTHROPIC_KEY"
    );
    assert_eq!(before["providers"]["anthropic"]["baseUrl"], "");
    assert!(!before.to_string().contains("existing-provider-value"));
    assert!(
        before["settingSources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| !source
                .as_str()
                .unwrap()
                .contains(root.path().to_str().unwrap()))
    );
    let updated = conn
        .send_request(
            methods::DESKTOP_UPDATE_SETTINGS.name,
            json!({
                "workspaceId": "local-workspace", "settings": settings("after"),
                "providerUpdates": {"openai": {
                    "enabled": true, "baseUrl": "https://proxy.example.test/v1",
                    "apiKeyEnv": "LOOPAL_OPENAI_KEY", "apiKey": "new-write-only-value"
                }}
            }),
        )
        .await
        .unwrap();
    assert_eq!(updated["settings"], settings("after"));
    assert_eq!(updated["providers"]["openai"]["apiKeyConfigured"], true);
    assert!(!updated.to_string().contains("existing-provider-value"));
    assert!(!updated.to_string().contains("new-write-only-value"));
    let entries = updated["resolvedEntries"].as_array().unwrap();
    assert!(entries.iter().any(|entry| {
        entry["key"] == "providers.openai.api_key" && entry["value"] == "********"
    }));
    assert!(entries.iter().any(|entry| {
        entry["key"] == "providers.openai.api_key_env" && entry["value"] == "********"
    }));
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(user_dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(raw["model"], "after");
    assert_eq!(
        raw["providers"]["anthropic"]["api_key"],
        "existing-provider-value"
    );
    assert_eq!(raw["providers"]["anthropic"]["unknown"], true);
    assert_eq!(
        raw["providers"]["openai"]["api_key"],
        "new-write-only-value"
    );
    assert_eq!(
        raw["model_routing"]["summarization"],
        serde_json::Value::Null
    );
    assert_eq!(raw["model_routing"]["classification"], "classifier-model");
    assert_eq!(raw["unknown_top_level"]["preserve"], true);
    assert_eq!(
        raw["sandbox"]["network"]["denied_domains"][0],
        "blocked.test"
    );
    assert_eq!(raw["sandbox"]["policy"], "read_only");
    let project: serde_json::Value =
        serde_json::from_slice(&std::fs::read(project_dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(project["model"], "project-override");
    let reread = conn
        .send_request(
            methods::DESKTOP_GET_SETTINGS.name,
            json!({"workspaceId": "local-workspace"}),
        )
        .await
        .unwrap();
    assert_eq!(reread["settings"], settings("after"));
    let cleared = conn
        .send_request(
            methods::DESKTOP_UPDATE_SETTINGS.name,
            json!({
                "workspaceId": "local-workspace", "settings": settings("after"),
                "providerUpdates": {"openai": {"clearApiKey": true}}
            }),
        )
        .await
        .unwrap();
    assert_eq!(cleared["providers"]["openai"]["apiKeyConfigured"], false);
    assert!(!cleared.to_string().contains("existing-provider-value"));
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(user_dir.join("settings.json")).unwrap()).unwrap();
    assert!(raw["providers"]["openai"]["api_key"].is_null());
    assert_eq!(
        raw["providers"]["anthropic"]["api_key"],
        "existing-provider-value"
    );
}
