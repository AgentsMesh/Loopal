use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use loopal_hub_vault::{AuditContext, HubVaultService};
use loopal_secret_client::SecretError;
use loopal_vault_age::{AgeVault, DiscoveredIdentity, Recipients};
use loopal_vault_api::{
    AuditError, AuditMetadata, AuditResult, AuditSink, ProtectedOp, Vault, VaultOp,
};
use secrecy::SecretString;
use tempfile::TempDir;

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

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    identity: Arc<DiscoveredIdentity>,
}

async fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().to_path_buf();
    let vault_dir = cwd.join(".loopal/vaults/default.vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    let recipients_path = vault_dir.join("recipients");
    let mut recipients = Recipients::new();
    recipients.add_line(PUBLIC_KEY).unwrap();
    recipients.write(&recipients_path).unwrap();
    let key_path = cwd.join("id_ed25519");
    write_key(&key_path, PRIVATE_KEY);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let vault = AgeVault::new(
        vault_dir.join("store.age"),
        recipients_path,
        identity.clone(),
    );
    vault.rekey().await.unwrap();
    vault
        .put("api_key", SecretString::from("plaintext"))
        .await
        .unwrap();
    Fixture {
        _temp: temp,
        cwd,
        identity,
    }
}

#[cfg(unix)]
fn write_key(path: &Path, value: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, value).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn write_key(path: &Path, value: &str) {
    std::fs::write(path, value).unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedAudit {
    key: String,
    session_id: Option<String>,
    cwd: PathBuf,
    agent_name: String,
    depth: u32,
    tool_name: Option<String>,
}

struct CapturingSink(Arc<Mutex<Vec<CapturedAudit>>>);

impl AuditSink for CapturingSink {
    fn record(&self, _op: VaultOp, key: &str, metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        self.0.lock().unwrap().push(CapturedAudit {
            key: key.into(),
            session_id: metadata.session_id.map(str::to_string),
            cwd: metadata.cwd.unwrap().to_path_buf(),
            agent_name: metadata.agent_name.unwrap().into(),
            depth: metadata.depth.unwrap(),
            tool_name: metadata.tool_name.map(str::to_string),
        });
        Ok(())
    }

    fn record_protected(
        &self,
        _op: ProtectedOp,
        _subject: &str,
        _metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        Ok(())
    }
}

struct FailingSink;

impl AuditSink for FailingSink {
    fn record(&self, _op: VaultOp, _key: &str, _metadata: &AuditMetadata<'_>) -> AuditResult<()> {
        Err(AuditError::Serialization("forced failure".into()))
    }

    fn record_protected(
        &self,
        _op: ProtectedOp,
        _subject: &str,
        _metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        Err(AuditError::Serialization("forced failure".into()))
    }
}

fn context() -> AuditContext {
    AuditContext {
        session_id: Some("authenticated-session".into()),
        agent_name: "authenticated-agent".into(),
        depth: 3,
        tool_name: Some("Bash".into()),
    }
}

#[tokio::test]
async fn hub_get_records_context_once() {
    let fixture = fixture().await;
    let entries = Arc::new(Mutex::new(Vec::new()));
    let service = HubVaultService::with_identity(
        fixture.identity.clone(),
        Arc::new(CapturingSink(entries.clone())),
    );

    service
        .get(&fixture.cwd, "api_key", context())
        .await
        .unwrap();

    let captured = entries.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].session_id.as_deref(),
        Some("authenticated-session")
    );
    assert_eq!(captured[0].cwd, fixture.cwd.canonicalize().unwrap());
    assert_eq!(captured[0].agent_name, "authenticated-agent");
    assert_eq!(captured[0].depth, 3);
    assert_eq!(captured[0].tool_name.as_deref(), Some("Bash"));
    assert_eq!(captured[0].key, "api_key");
}

#[tokio::test]
async fn missing_lookup_is_audited_once_on_cold_and_warm_cache() {
    let fixture = fixture().await;
    let entries = Arc::new(Mutex::new(Vec::new()));
    let service =
        HubVaultService::with_identity(fixture.identity, Arc::new(CapturingSink(entries.clone())));

    for name in ["cold_missing", "warm_missing"] {
        let error = service
            .get(&fixture.cwd, name, context())
            .await
            .unwrap_err();
        assert!(matches!(error, SecretError::SecretNotFound(ref found) if found == name));
    }

    let captured = entries.lock().unwrap();
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0].key, "cold_missing");
    assert_eq!(captured[1].key, "warm_missing");
}

#[tokio::test]
async fn hub_get_surfaces_audit_failure_without_plaintext() {
    let fixture = fixture().await;
    let service = HubVaultService::with_identity(fixture.identity, Arc::new(FailingSink));

    let error = service
        .get(&fixture.cwd, "api_key", context())
        .await
        .unwrap_err();

    assert!(matches!(error, SecretError::DecryptFailed(_)));
    assert!(error.to_string().contains("protected audit failed"));
    assert!(!error.to_string().contains("plaintext"));
}
