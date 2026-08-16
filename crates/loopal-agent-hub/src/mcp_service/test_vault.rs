use std::sync::Arc;

use loopal_hub_vault::HubVaultService;
use loopal_vault_age::{AgeVault, Recipients};
use loopal_vault_api::Vault;

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

pub(crate) async fn service(entries: &[(&str, &str)]) -> (tempfile::TempDir, Arc<HubVaultService>) {
    let temp = tempfile::tempdir().unwrap();
    let key_path = temp.path().join("id_ed25519");
    std::fs::write(&key_path, PRIVATE_KEY).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let vault_dir = temp.path().join(".loopal/vaults/default.vault");
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
    (temp, Arc::new(service))
}
