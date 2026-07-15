#[tokio::test]
async fn desktop_settings_rpc_manages_compatible_provider_secrets() {
    let root = tempfile::tempdir().unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let updated = conn
        .send_request(
            methods::DESKTOP_UPDATE_SETTINGS.name,
            json!({
                "workspaceId": "local-workspace", "settings": settings("compat/model"),
                "providerUpdates": {"openaiCompatible": [{
                    "name": "desktop-compat", "baseUrl": "https://compat.example.test/v1",
                    "modelPrefix": "compat/", "apiKeyEnv": "", "apiKey": "compat-secret"
                }]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        updated["openaiCompatible"][0],
        json!({
            "name": "desktop-compat", "baseUrl": "https://compat.example.test/v1",
            "modelPrefix": "compat/", "apiKeyEnv": "", "apiKeyConfigured": true
        })
    );
    assert!(
        updated["configuredProviders"]
            .as_array()
            .unwrap()
            .contains(&json!("openai-compatible: desktop-compat"))
    );
    assert!(!updated.to_string().contains("compat-secret"));

    let preserved = conn
        .send_request(
            methods::DESKTOP_UPDATE_SETTINGS.name,
            json!({
                "workspaceId": "local-workspace", "settings": settings("compat/model"),
                "providerUpdates": {"openaiCompatible": [{
                    "name": "desktop-compat", "modelPrefix": "compat-v2/"
                }]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        preserved["openaiCompatible"][0]["modelPrefix"],
        "compat-v2/"
    );
    let path = root.path().join(".loopal-user/settings.json");
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        raw["providers"]["openai_compat"][0]["api_key"],
        "compat-secret"
    );

    let cleared = conn
        .send_request(
            methods::DESKTOP_UPDATE_SETTINGS.name,
            json!({
                "workspaceId": "local-workspace", "settings": settings("compat/model"),
                "providerUpdates": {"openaiCompatible": [{
                    "name": "desktop-compat", "clearApiKey": true
                }]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(cleared["openaiCompatible"][0]["apiKeyConfigured"], false);
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("compat-secret")
    );

    let removed = conn
        .send_request(
            methods::DESKTOP_UPDATE_SETTINGS.name,
            json!({
                "workspaceId": "local-workspace", "settings": settings("compat/model"),
                "providerUpdates": {"openaiCompatible": [{
                    "name": "desktop-compat", "remove": true
                }]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(removed["openaiCompatible"], json!([]));
}
