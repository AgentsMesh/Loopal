use std::sync::Arc;

use loopal_vault_api::{AuditMetadata, AuditResult, AuditSink, ProtectedOp, VaultOp};
use tempfile::TempDir;
use tokio::sync::{Mutex, mpsc};

use super::*;

const PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQAAAJCfEwtqnxML
agAAAAtzc2gtZWQyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQ
AAAEADBJvjZT8X6JRJI8xVq/1aU8nMVgOtVnmdwqWwrSlXG3sKLqeplhpW+uObz5dvMgjz
1OxfM/XXUB+VHtZ6isGNAAAADHN0cjRkQGNhcmJvbgE=
-----END OPENSSH PRIVATE KEY-----
";

struct RecordingAudit;

impl AuditSink for RecordingAudit {
    fn record(&self, _: VaultOp, _: &str, _: &AuditMetadata<'_>) -> AuditResult<()> {
        Ok(())
    }

    fn record_protected(&self, _: ProtectedOp, _: &str, _: &AuditMetadata<'_>) -> AuditResult<()> {
        Ok(())
    }
}

fn config(audit_dir: &std::path::Path) -> loopal_config::ResolvedConfig {
    let settings = loopal_config::Settings {
        harness: loopal_config::HarnessConfig {
            agent_max_total: 7,
            agent_max_depth: 4,
            ..Default::default()
        },
        telemetry: loopal_config::TelemetryConfig {
            telemetry_dir: Some(audit_dir.to_string_lossy().into_owned()),
            ..Default::default()
        },
        ..Default::default()
    };
    loopal_config::ResolvedConfig {
        settings,
        workflow_preset_thinking_recommendation: None,
        mcp_servers: Default::default(),
        skills: Default::default(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        classifier_prompt: None,
        layers: Vec::new(),
        secrets: None,
    }
}

fn hub(cwd: &std::path::Path) -> Arc<Mutex<Hub>> {
    let (tx, _rx) = mpsc::channel(1);
    Arc::new(Mutex::new(Hub::with_cwd(tx, cwd.to_path_buf())))
}

#[tokio::test]
async fn production_build_initializes_jsonl_audit_without_noop_default() {
    let cwd = TempDir::new().unwrap();
    let audit_dir = cwd.path().join("audit");
    let config = config(&audit_dir);
    let built = HubBuilt::new(cwd.path(), &config).await.unwrap();

    assert!(audit_dir.join("secret_access.jsonl").is_file());
    assert!(built.hub.lock().await.protected_audit.is_some());
}

#[tokio::test]
async fn production_build_fails_closed_when_audit_initialization_fails() {
    let cwd = TempDir::new().unwrap();
    let blocker = cwd.path().join("audit-blocker");
    std::fs::write(&blocker, "occupied").unwrap();
    let config = config(&blocker);
    let error = match HubBuilt::new(cwd.path(), &config).await {
        Ok(_) => panic!("protected audit failure must abort Hub construction"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("protected audit initialization failed")
    );
}

#[tokio::test]
async fn build_applies_limits_and_wires_successful_services() {
    let cwd = TempDir::new().unwrap();
    let config = config(cwd.path());
    let built = HubBuilt::new_with(
        cwd.path(),
        &config,
        |_| Ok(Arc::new(RecordingAudit)),
        |audit| {
            let key = cwd.path().join("id_ed25519");
            write_key(&key);
            let identity = Arc::new(loopal_vault_age::load(&key).unwrap());
            Ok(loopal_hub_vault::HubVaultService::with_identity(
                identity, audit,
            ))
        },
    )
    .await
    .unwrap();

    let locked = built.hub.lock().await;
    assert_eq!(locked.max_total_agents, 7);
    assert_eq!(locked.max_agent_depth, 4);
    assert!(locked.protected_audit.is_some());
    assert!(locked.vault_service.is_some());
}

#[cfg(unix)]
fn write_key(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, PRIVATE_KEY).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[cfg(not(unix))]
fn write_key(path: &std::path::Path) {
    std::fs::write(path, PRIVATE_KEY).unwrap();
}

#[tokio::test]
async fn audit_failure_aborts_before_vault_construction() {
    let cwd = TempDir::new().unwrap();
    let hub = hub(cwd.path());
    let error = init_vault_with(&hub, Err("audit blocked".into()), |_| {
        panic!("vault constructor must not run")
    })
    .await
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("protected audit initialization failed")
    );
    let locked = hub.lock().await;
    assert!(locked.protected_audit.is_none());
    assert!(locked.vault_service.is_none());
}

#[tokio::test]
async fn vault_failure_keeps_real_audit_and_disables_vault() {
    let cwd = TempDir::new().unwrap();
    let hub = hub(cwd.path());
    let audit: Arc<dyn AuditSink> = Arc::new(RecordingAudit);
    let outcome = init_vault_with(&hub, Ok(audit.clone()), |_| Err("identity missing".into()))
        .await
        .unwrap();

    assert_eq!(outcome, VaultInit::VaultUnavailable);
    let locked = hub.lock().await;
    assert!(locked.protected_audit.is_some());
    assert!(locked.vault_service.is_none());
}
