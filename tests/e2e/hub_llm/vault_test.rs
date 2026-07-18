use std::sync::Arc;

use secrecy::SecretString;
use serde_json::json;

use crate::support::{HubEnv, HubHarness};

const SSH_PUBKEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN e2e@test";
const SSH_PRIVKEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQAAAJCfEwtqnxML
agAAAAtzc2gtZWQyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQ
AAAEADBJvjZT8X6JRJI8xVq/1aU8nMVgOtVnmdwqWwrSlXG3sKLqeplhpW+uObz5dvMgjz
1OxfM/XXUB+VHtZ6isGNAAAADHN0cjRkQGNhcmJvbgE=
-----END OPENSSH PRIVATE KEY-----
";

/// Seed a REAL age-encrypted vault into the Hub's project before launch: SSH
/// identity under HOME/.ssh, `<cwd>/.loopal/vaults/main.vault` holding one
/// secret. This is the actual production secret store, not a harness stub.
async fn seed_vault(env: &HubEnv, name: &str, value: &str) {
    let ssh_dir = env.home.path().join(".ssh");
    std::fs::create_dir_all(&ssh_dir).unwrap();
    let key_path = ssh_dir.join("id_ed25519");
    std::fs::write(&key_path, SSH_PRIVKEY).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(&key_path).unwrap().permissions();
        p.set_mode(0o600);
        std::fs::set_permissions(&key_path, p).unwrap();
    }

    let vault_dir = env.cwd.path().join(".loopal/vaults/main.vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    let recipients_path = vault_dir.join("recipients");
    let mut recipients = loopal_vault_age::Recipients::new();
    recipients.add_line(SSH_PUBKEY).unwrap();
    recipients.write(&recipients_path).unwrap();
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let vault =
        loopal_vault_age::AgeVault::new(vault_dir.join("store.age"), recipients_path, identity);
    use loopal_vault_api::Vault as _;
    vault
        .put(name, SecretString::from(value.to_string()))
        .await
        .unwrap();
}

/// The complete production secret chain: `<secret_ref:NAME>` in a Bash call
/// resolves through the Hub's REAL vault service (age decryption of the
/// on-disk store), the shell runs with the plaintext (proved by length), and
/// the result is redacted before the LLM wire ever sees it.
#[tokio::test]
async fn secret_resolves_from_a_real_age_vault_and_redacts() {
    let env = HubEnv::new();
    seed_vault(&env, "e2e_token", "hub-vault-plain").await;
    let mut h = HubHarness::launch(
        env,
        json!({
            "version": 2,
            "name": "hub_vault",
            "calls": [
                {"expect": {"userContains": "use the vault secret"},
                 "chunks": [
                    {"type": "tool_use", "id": "v1", "name": "Bash",
                     "input": {"command":
                        "echo 'tag-<secret_ref:e2e_token>-tag'; \
                         test \"$(printf %s '<secret_ref:e2e_token>' | wc -c)\" -eq 15 \
                         && echo len-ok || echo len-bad"}},
                    {"type": "done"}
                 ]},
                {"expect": {"toolResultId": "v1"},
                 "chunks": [{"type": "text", "text": "vault secret handled"}, {"type": "done"}]}
            ],
            "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
        }),
        false,
    )
    .await;

    let out = h.turn("please use the vault secret").await;
    assert!(
        out.error.is_none() && out.finished && out.text.contains("vault secret handled"),
        "turn failed: {out:?}"
    );
    let result = out
        .events
        .iter()
        .find(|e| e.starts_with("ToolResult"))
        .expect("a Bash ToolResult");
    assert!(
        result.contains("len-ok"),
        "len-ok proves the shell saw the 15-char plaintext decrypted from the \
         age store; result: {result}"
    );
    assert!(
        result.contains("tag-<secret_ref:e2e_token>-tag"),
        "the echoed plaintext must be redacted back to the wire form; \
         result: {result}"
    );
    assert!(
        !out.events.iter().any(|e| e.contains("hub-vault-plain")),
        "plaintext must never appear in any event; events: {:?}",
        out.events
    );
    let journal = h.journal().await.to_string();
    assert!(
        !journal.contains("hub-vault-plain"),
        "the LLM wire must only ever see placeholders; journal: {journal}"
    );
}
