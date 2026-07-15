#[tokio::test]
async fn desktop_settings_rpc_rejects_invalid_or_extra_fields_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let mut invalid = settings("safe");
    invalid["permissionMode"] = json!("unrestricted");
    invalid["apiKey"] = json!("must-not-cross");
    conn.send_request(
        methods::DESKTOP_UPDATE_SETTINGS.name,
        json!({"workspaceId": "local-workspace", "settings": invalid}),
    )
    .await
    .expect_err("strict settings input must fail");
    let mut nested = settings("safe");
    nested["thinking"] = json!({"type": "auto", "api_key": "must-not-cross"});
    conn.send_request(
        methods::DESKTOP_UPDATE_SETTINGS.name,
        json!({"workspaceId": "local-workspace", "settings": nested}),
    )
    .await
    .expect_err("unknown nested settings fields must fail");
    conn.send_request(
        methods::DESKTOP_UPDATE_SETTINGS.name,
        json!({
            "workspaceId": "local-workspace", "settings": settings("safe"),
            "providerUpdates": {"openai": {
                "baseUrl": "https://user:password@example.test/?token=value"
            }}
        }),
    )
    .await
    .expect_err("credential-bearing provider URL must fail");
    conn.send_request(
        methods::DESKTOP_UPDATE_SETTINGS.name,
        json!({
            "workspaceId": "local-workspace", "settings": settings("safe"),
            "providerUpdates": {"openaiCompatible": [{
                "name": "bad", "baseUrl": "https://user:password@example.test/v1"
            }]}
        }),
    )
    .await
    .expect_err("credential-bearing compatible URL must fail");
    assert!(!root.path().join(".loopal-user/settings.json").exists());
}

#[tokio::test]
async fn desktop_settings_rpc_enforces_ui_and_workspace_acl() {
    let root = tempfile::tempdir().unwrap();
    let (hub, conn, _rx) = setup(root.path()).await;
    conn.send_request(
        methods::DESKTOP_GET_SETTINGS.name,
        json!({"workspaceId": "outside-workspace"}),
    )
    .await
    .expect_err("cross-workspace settings read must fail");
    conn.send_request(
        methods::DESKTOP_UPDATE_SETTINGS.name,
        json!({"workspaceId": "outside-workspace", "settings": settings("blocked")}),
    )
    .await
    .expect_err("cross-workspace settings write must fail");
    let denied = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::DESKTOP_GET_SETTINGS.name,
        json!({"workspaceId": "local-workspace"}),
        "agent-worker".into(),
    )
    .await
    .unwrap_err();
    assert!(denied.contains("require a UI client"), "{denied}");
    hub.lock().await.user_config_dir = None;
    let unavailable = conn
        .send_request(
            methods::DESKTOP_GET_SETTINGS.name,
            json!({"workspaceId": "local-workspace"}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(unavailable.contains("user configuration is unavailable"));
}
