use std::sync::Arc;

use loopal_hub_vault::HubVaultService;
use loopal_secret_client::{ExposeSecret, HUB_RPC_BUDGET, SecretClient, SecretError};
use loopal_vault_age::{AgeVault, Recipients};
use loopal_vault_api::Vault;

use super::HubMcpSecretClient;

const PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQAAAJCfEwtqnxML
agAAAAtzc2gtZWQyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQ
AAAEADBJvjZT8X6JRJI8xVq/1aU8nMVgOtVnmdwqWwrSlXG3sKLqeplhpW+uObz5dvMgjz
1OxfM/XXUB+VHtZ6isGNAAAADHN0cjRkQGNhcmJvbgE=
-----END OPENSSH PRIVATE KEY-----
";
const PUBLIC_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";

async fn client(entries: &[(&str, &str)]) -> (tempfile::TempDir, HubMcpSecretClient) {
    let tmp = tempfile::tempdir().unwrap();
    let key_path = tmp.path().join("id_ed25519");
    std::fs::write(&key_path, PRIVATE_KEY).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let vault_dir = tmp.path().join(".loopal/vaults/default.vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    let recipients_path = vault_dir.join("recipients");
    let mut recipients = Recipients::new();
    recipients.add_line(PUBLIC_KEY).unwrap();
    recipients.write(&recipients_path).unwrap();
    let vault = AgeVault::new(
        vault_dir.join("store.age"),
        recipients_path,
        identity.clone(),
    );
    vault.rekey().await.unwrap();
    for (name, value) in entries {
        vault
            .put(name, secrecy::SecretString::from((*value).to_string()))
            .await
            .unwrap();
    }
    let service =
        HubVaultService::with_identity(identity, Arc::new(loopal_vault_api::NoopAuditSink));
    let client = HubMcpSecretClient::new(Arc::new(service), tmp.path().to_path_buf());
    (tmp, client)
}

#[tokio::test]
async fn delegates_get_list_and_both_placeholder_syntaxes() {
    let (_tmp, client) = client(&[("api_key", "secret-value"), ("token", "wire-value")]).await;

    assert_eq!(
        client
            .get("api_key", HUB_RPC_BUDGET)
            .await
            .unwrap()
            .expose_secret(),
        "secret-value"
    );
    let mut names = client.list_names(HUB_RPC_BUDGET).await.unwrap();
    names.sort();
    assert_eq!(names, ["api_key", "token"]);
    assert_eq!(
        client
            .expand_author("key={{secret:api_key}}", HUB_RPC_BUDGET)
            .await
            .unwrap()
            .expose_secret(),
        "key=secret-value"
    );
    assert_eq!(
        client
            .expand_wire("Bearer <secret_ref:token>", HUB_RPC_BUDGET)
            .await
            .unwrap()
            .expose_secret(),
        "Bearer wire-value"
    );
    let snapshot = client
        .final_sink_redaction_seed()
        .unwrap()
        .snapshot()
        .unwrap();
    assert!(snapshot.iter().any(|(name, _)| name == "api_key"));
    assert!(snapshot.iter().any(|(name, _)| name == "token"));
}

#[tokio::test]
async fn missing_secret_preserves_structured_error() {
    let (_tmp, client) = client(&[("present", "value")]).await;

    let error = client.get("missing", HUB_RPC_BUDGET).await.unwrap_err();
    assert!(matches!(error, SecretError::SecretNotFound(name) if name == "missing"));
}
