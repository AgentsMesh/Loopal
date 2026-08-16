use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_vault_api::{NoopAuditSink, SecretString, VaultResult};

use super::*;

const SSH_PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQAAAJCfEwtqnxML
agAAAAtzc2gtZWQyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQ
AAAEADBJvjZT8X6JRJI8xVq/1aU8nMVgOtVnmdwqWwrSlXG3sKLqeplhpW+uObz5dvMgjz
1OxfM/XXUB+VHtZ6isGNAAAADHN0cjRkQGNhcmJvbgE=
-----END OPENSSH PRIVATE KEY-----
";

struct MockVault;

#[async_trait]
impl Vault for MockVault {
    async fn get(&self, _name: &str) -> Option<SecretString> {
        None
    }

    async fn list_names(&self) -> Vec<String> {
        Vec::new()
    }

    async fn put(&self, _name: &str, _value: SecretString) -> VaultResult<()> {
        Ok(())
    }

    async fn delete(&self, _name: &str) -> VaultResult<()> {
        Ok(())
    }

    async fn rekey(&self) -> VaultResult<()> {
        Ok(())
    }
}

type StoreResult = Result<Option<Arc<dyn Vault>>, ConfigError>;

fn assemble(
    names: &[&str],
    requested: Option<&str>,
    audit_ok: bool,
    identity: Result<(), &'static str>,
) -> (StoreResult, Vec<String>) {
    let listed = names.iter().map(|name| (*name).to_string()).collect();
    let constructed = Arc::new(Mutex::new(Vec::new()));
    let captured = constructed.clone();
    let result = build_secret_store_with(
        Some(PathBuf::from("/vaults")),
        requested,
        PathBuf::from("/audit"),
        move |_| listed,
        move |_| {
            if audit_ok {
                Ok(Arc::new(NoopAuditSink) as Arc<dyn AuditSink>)
            } else {
                Err(ConfigError::InvalidValue {
                    field: "telemetry.telemetry_dir".into(),
                    reason: "unavailable".into(),
                })
            }
        },
        move || identity,
        move |_, name, _, _| {
            captured.lock().unwrap().push(name.to_string());
            Arc::new(MockVault)
        },
    );
    let names = constructed.lock().unwrap().clone();
    (result, names)
}

#[test]
fn absent_or_empty_vault_directory_returns_none() {
    let absent = build_secret_store_with::<(), &str>(
        None,
        None,
        PathBuf::new(),
        |_| panic!("must not list"),
        |_| panic!("must not open audit"),
        || panic!("must not discover identity"),
        |_, _, _, _| panic!("must not create vault"),
    )
    .unwrap();
    assert!(absent.is_none());

    let (empty, constructed) = assemble(&[], None, true, Ok(()));
    assert!(empty.unwrap().is_none());
    assert!(constructed.is_empty());
}

#[test]
fn default_and_requested_vaults_are_selected_first() {
    let (implicit, made) = assemble(&["alpha", "default", "zeta"], None, true, Ok(()));
    assert!(implicit.unwrap().is_some());
    assert_eq!(made, ["default", "alpha", "zeta"]);

    let (requested, made) = assemble(&["alpha", "zeta"], Some("zeta"), true, Ok(()));
    assert!(requested.unwrap().is_some());
    assert_eq!(made, ["zeta", "alpha"]);
}

#[test]
fn first_alphabetical_vault_is_implicit_fallback() {
    let (result, made) = assemble(&["alpha", "zeta"], None, true, Ok(()));
    assert!(result.unwrap().is_some());
    assert_eq!(made, ["alpha", "zeta"]);
}

#[test]
fn explicit_missing_default_is_a_configuration_error() {
    let (result, made) = assemble(&["alpha", "zeta"], Some("missing"), true, Ok(()));
    let Err(ConfigError::InvalidValue { field, reason }) = result else {
        panic!("expected invalid value");
    };
    assert_eq!(field, "secrets.default_vault");
    assert!(reason.contains("missing"));
    assert!(reason.contains("alpha, zeta"));
    assert!(made.is_empty());
}

#[test]
fn audit_and_identity_failures_stop_vault_construction() {
    let (audit, made) = assemble(&["default"], None, false, Ok(()));
    let Err(error) = audit else {
        panic!("expected audit error");
    };
    assert!(error.to_string().contains("unavailable"));
    assert!(made.is_empty());

    let (identity, made) = assemble(&["default"], None, true, Err("no identity"));
    assert!(identity.unwrap().is_none());
    assert!(made.is_empty());
}

#[test]
fn single_vault_returns_without_merge() {
    let (result, made) = assemble(&["default"], None, true, Ok(()));
    assert!(result.unwrap().is_some());
    assert_eq!(made, ["default"]);
}

#[cfg(unix)]
#[test]
fn age_vault_factory_assembles_store_paths() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    std::fs::write(&key_path, SSH_PRIVATE_KEY).unwrap();
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());

    let _vault = make_age_vault(dir.path(), "default", &identity, Arc::new(NoopAuditSink));
}
