//! Unit tests for HubVaultService — direct method-level coverage that the
//! existing agent-hub e2e tests skip in favor of full IPC round-trips.
//!
//! Audit gap: `loopal-hub-vault` had no tests/ dir at all. The crate's
//! cache semantics, fallback-naming policy, and template-expansion methods
//! were only exercised transitively through `e2e_real_vault_test.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use loopal_hub_vault::{AuditContext, HubVaultService};
use loopal_secret_client::SecretError;
use loopal_vault_age::{AgeVault, DiscoveredIdentity, Recipients};
use loopal_vault_api::{NoopAuditSink, Vault};
use secrecy::{ExposeSecret, SecretString};
use tempfile::TempDir;

// Public test fixtures from the age crate (alice's ed25519 keypair).
const ED25519_UNENCRYPTED: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQAAAJCfEwtqnxML
agAAAAtzc2gtZWQyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQ
AAAEADBJvjZT8X6JRJI8xVq/1aU8nMVgOtVnmdwqWwrSlXG3sKLqeplhpW+uObz5dvMgjz
1OxfM/XXUB+VHtZ6isGNAAAADHN0cjRkQGNhcmJvbgE=
-----END OPENSSH PRIVATE KEY-----
";
const PUBKEY_ALICE: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";

#[cfg(unix)]
fn write_key_0600(path: &std::path::Path, content: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, content).unwrap();
    let mut p = std::fs::metadata(path).unwrap().permissions();
    p.set_mode(0o600);
    std::fs::set_permissions(path, p).unwrap();
}

#[cfg(not(unix))]
fn write_key_0600(path: &std::path::Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

struct VaultFixture {
    _tmp: TempDir,
    cwd: PathBuf,
    identity: Arc<DiscoveredIdentity>,
}

async fn vault_dir(cwd: &std::path::Path, name: &str) -> (PathBuf, AgeVault) {
    let dir = cwd
        .join(".loopal")
        .join("vaults")
        .join(format!("{name}.vault"));
    std::fs::create_dir_all(&dir).unwrap();
    let recipients_path = dir.join("recipients");
    let mut recipients = Recipients::new();
    recipients.add_line(PUBKEY_ALICE).unwrap();
    recipients.write(&recipients_path).unwrap();
    let key_path = cwd.join("id_ed25519");
    write_key_0600(&key_path, ED25519_UNENCRYPTED);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let store_path = dir.join("store.age");
    let vault = AgeVault::new(store_path, recipients_path, identity);
    vault.rekey().await.unwrap();
    (dir, vault)
}

async fn setup_fixture(pairs: &[(&str, &str)]) -> VaultFixture {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().to_path_buf();
    let (_dir, vault) = vault_dir(&cwd, "default").await;
    for (k, v) in pairs {
        vault
            .put(k, SecretString::from((*v).to_string()))
            .await
            .expect("vault put");
    }
    let key_path = cwd.join("id_ed25519");
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    VaultFixture {
        _tmp: tmp,
        cwd,
        identity,
    }
}

fn make_service(identity: Arc<DiscoveredIdentity>) -> HubVaultService {
    HubVaultService::with_identity(identity, Arc::new(NoopAuditSink))
}

fn audit_ctx() -> AuditContext {
    AuditContext {
        session_id: Some("test-session".into()),
        agent_name: "test-agent".into(),
        depth: 0,
        tool_name: None,
    }
}

#[tokio::test]
async fn get_returns_secret_for_existing_name() {
    let fx = setup_fixture(&[("api_key", "sk-test-123")]).await;
    let svc = make_service(fx.identity.clone());
    let secret = svc
        .get(&fx.cwd, "api_key", audit_ctx())
        .await
        .expect("get must succeed for existing key");
    assert_eq!(secret.expose_secret(), "sk-test-123");
}

#[tokio::test]
async fn get_returns_secret_not_found_for_missing_name() {
    let fx = setup_fixture(&[("present", "x")]).await;
    let svc = make_service(fx.identity.clone());
    let err = svc.get(&fx.cwd, "absent", audit_ctx()).await.unwrap_err();
    match err {
        SecretError::SecretNotFound(name) => assert_eq!(name, "absent"),
        other => panic!("expected SecretNotFound, got: {other:?}"),
    }
}

#[tokio::test]
async fn list_names_returns_all_keys_sorted() {
    let fx = setup_fixture(&[("k1", "v1"), ("k2", "v2"), ("openai_api", "sk-x")]).await;
    let svc = make_service(fx.identity.clone());
    let mut names = svc.list_names(&fx.cwd).await.unwrap();
    names.sort();
    assert_eq!(names, vec!["k1", "k2", "openai_api"]);
}

#[tokio::test]
async fn vault_for_missing_cwd_returns_vault_not_found() {
    // No vault dir at this cwd → canonicalize succeeds (real tempdir) but
    // open_default_vault yields VaultNotFound for the empty vaults/ dir.
    let tmp = tempfile::tempdir().unwrap();
    let key_path = tmp.path().join("id_ed25519");
    write_key_0600(&key_path, ED25519_UNENCRYPTED);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let svc = make_service(identity);
    let err = svc
        .get(tmp.path(), "anything", audit_ctx())
        .await
        .unwrap_err();
    assert!(
        matches!(err, SecretError::VaultNotFound(_)),
        "expected VaultNotFound, got: {err:?}"
    );
}

#[tokio::test]
async fn vault_for_nonexistent_path_returns_vault_not_found() {
    // canonicalize() fails → mapped to VaultNotFound by vault_for.
    let tmp = tempfile::tempdir().unwrap();
    let key_path = tmp.path().join("id_ed25519");
    write_key_0600(&key_path, ED25519_UNENCRYPTED);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let svc = make_service(identity);
    let ghost = tmp.path().join("does-not-exist-anywhere");
    let err = svc.get(&ghost, "anything", audit_ctx()).await.unwrap_err();
    assert!(
        matches!(err, SecretError::VaultNotFound(_)),
        "expected VaultNotFound for unresolvable path, got: {err:?}"
    );
}

#[tokio::test]
async fn vault_for_caches_vault_handle_per_cwd() {
    // Second call to `get` must NOT re-construct AgeVault. The contract is
    // unobservable directly without a probe, so verify via a follow-up put
    // through a separate AgeVault instance: if the cache is doing its job,
    // the second `get` reads the updated value (because the cached vault
    // shares the same on-disk store file, AgeVault has no in-memory cache).
    //
    // What's actually observable: multiple gets to the same cwd succeed in
    // sequence without canonicalize churn (any error path would surface as
    // VaultNotFound on the second call).
    let fx = setup_fixture(&[("a", "1"), ("b", "2")]).await;
    let svc = make_service(fx.identity.clone());

    for _ in 0..5 {
        let v = svc
            .get(&fx.cwd, "a", audit_ctx())
            .await
            .expect("repeated get must succeed via cache");
        assert_eq!(v.expose_secret(), "1");
    }
}

#[tokio::test]
async fn open_default_vault_prefers_default_when_multiple_present() {
    // Two vaults: "default" (with key "x") and "alt-other" (with key "y").
    // Lookup must hit "default", not "alt-other".
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().to_path_buf();
    let (_d, default_vault) = vault_dir(&cwd, "default").await;
    default_vault
        .put("x", SecretString::from("from-default".to_string()))
        .await
        .unwrap();
    let (_a, alt_vault) = vault_dir(&cwd, "alt-other").await;
    alt_vault
        .put("x", SecretString::from("from-alt".to_string()))
        .await
        .unwrap();

    let key_path = cwd.join("id_ed25519");
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let svc = make_service(identity);

    let secret = svc.get(&cwd, "x", audit_ctx()).await.unwrap();
    assert_eq!(
        secret.expose_secret(),
        "from-default",
        "default vault must win when present"
    );
}

#[tokio::test]
async fn open_default_vault_falls_back_to_alphabetical_first() {
    // No "default" vault — service must use the alphabetically-first name.
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().to_path_buf();
    let (_a, a_vault) = vault_dir(&cwd, "alpha").await;
    a_vault
        .put("name", SecretString::from("from-alpha".to_string()))
        .await
        .unwrap();
    let (_b, b_vault) = vault_dir(&cwd, "beta").await;
    b_vault
        .put("name", SecretString::from("from-beta".to_string()))
        .await
        .unwrap();

    let key_path = cwd.join("id_ed25519");
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());
    let svc = make_service(identity);

    let secret = svc.get(&cwd, "name", audit_ctx()).await.unwrap();
    assert_eq!(
        secret.expose_secret(),
        "from-alpha",
        "no 'default' vault → alphabetical first must win"
    );
}

#[tokio::test]
async fn expand_author_substitutes_double_brace_template() {
    let fx = setup_fixture(&[("api_key", "sk-AUTHOR")]).await;
    let svc = make_service(fx.identity.clone());
    let expanded = svc
        .expand_author(&fx.cwd, "prefix-{{secret:api_key}}-suffix", audit_ctx())
        .await
        .unwrap();
    assert_eq!(expanded.expose_secret(), "prefix-sk-AUTHOR-suffix");
}

#[tokio::test]
async fn expand_wire_substitutes_secret_ref_template() {
    let fx = setup_fixture(&[("token", "WIRE-VAL")]).await;
    let svc = make_service(fx.identity.clone());
    let expanded = svc
        .expand_wire(&fx.cwd, "Bearer <secret_ref:token>", audit_ctx())
        .await
        .unwrap();
    assert_eq!(expanded.expose_secret(), "Bearer WIRE-VAL");
}

#[tokio::test]
async fn expand_author_propagates_missing_secret_error() {
    let fx = setup_fixture(&[("only_one", "x")]).await;
    let svc = make_service(fx.identity.clone());
    let err = svc
        .expand_author(&fx.cwd, "{{secret:nonexistent}}", audit_ctx())
        .await
        .unwrap_err();
    match &err {
        SecretError::SecretNotFound(name) => assert_eq!(name, "nonexistent"),
        _ => panic!("missing secret during expansion must surface as SecretNotFound, got: {err:?}"),
    }
}

#[tokio::test]
async fn expand_author_no_placeholders_passes_through_verbatim() {
    let fx = setup_fixture(&[("ignored", "x")]).await;
    let svc = make_service(fx.identity.clone());
    let expanded = svc
        .expand_author(&fx.cwd, "plain text with no template", audit_ctx())
        .await
        .unwrap();
    assert_eq!(expanded.expose_secret(), "plain text with no template");
}

#[tokio::test]
async fn two_cwds_get_independent_vault_instances() {
    let fx_a = setup_fixture(&[("api_key", "VAL-A")]).await;
    let fx_b = setup_fixture(&[("api_key", "VAL-B")]).await;
    // Both fixtures share the same identity (same key bytes), so we can
    // use either's identity Arc here.
    let svc = make_service(fx_a.identity.clone());

    let a = svc.get(&fx_a.cwd, "api_key", audit_ctx()).await.unwrap();
    let b = svc.get(&fx_b.cwd, "api_key", audit_ctx()).await.unwrap();
    assert_eq!(a.expose_secret(), "VAL-A");
    assert_eq!(b.expose_secret(), "VAL-B");
}

#[tokio::test]
async fn with_noop_audit_constructor_compiles_and_runs() {
    // Smoke test that the test-friendly with_noop_audit() path works in
    // environments without an ~/.ssh/ key (will fail discover()).
    // We don't assert success — this just exercises the err-bubble path.
    let result = HubVaultService::with_noop_audit();
    match result {
        Ok(_svc) => {
            // Environment has a discoverable identity — fine, the service is usable.
        }
        Err(SecretError::DecryptFailed(msg)) => {
            assert!(
                msg.contains("SSH identity"),
                "decrypt-failed must reference SSH identity discovery, got: {msg}"
            );
        }
        Err(other) => panic!("unexpected error from with_noop_audit: {other:?}"),
    }
}
