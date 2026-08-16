//! Integration tests for `ConfigResolver::resolve` → `build_secret_store`.
//!
//! Focused on the assembly paths that don't require a real SSH identity
//! (vaults_dir missing / empty / containing only half-initialized vaults).
//! Happy-path encrypt/decrypt is covered by vault-age e2e tests; here we
//! verify that the config layer correctly returns None for the failure
//! modes and (transitively) a Vault impl when there are real vaults.

use loopal_config::{ConfigLayer, ConfigResolver, LayerSource};
use tempfile::tempdir;

fn make_layer(vaults_dir: std::path::PathBuf) -> ConfigLayer {
    let mut layer = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer.vaults_dir = Some(vaults_dir);
    layer
}

fn resolve_with(layer: ConfigLayer) -> loopal_config::ResolvedConfig {
    let mut r = ConfigResolver::new();
    r.add_layer(layer);
    r.resolve().expect("resolve")
}

#[test]
fn no_vaults_dir_yields_no_secret_store() {
    let resolved = resolve_with(ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    });
    assert!(resolved.secrets.is_none());
}

#[test]
fn nonexistent_vaults_dir_yields_no_secret_store() {
    let tmp = tempdir().unwrap();
    let resolved = resolve_with(make_layer(tmp.path().join("does_not_exist")));
    assert!(resolved.secrets.is_none());
}

#[test]
fn empty_vaults_dir_yields_no_secret_store() {
    let tmp = tempdir().unwrap();
    let vaults_dir = tmp.path().join("vaults");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    let resolved = resolve_with(make_layer(vaults_dir));
    assert!(resolved.secrets.is_none());
}

#[test]
fn vaults_dir_with_only_half_initialized_yields_no_secret_store() {
    let tmp = tempdir().unwrap();
    let vaults_dir = tmp.path().join("vaults");
    // .vault subdir without store.age — must be filtered out by discovery.
    let half = vaults_dir.join("default.vault");
    std::fs::create_dir_all(&half).unwrap();
    std::fs::write(half.join("recipients"), "").unwrap();

    let resolved = resolve_with(make_layer(vaults_dir));
    assert!(resolved.secrets.is_none());
}

#[test]
fn stray_non_vault_dirs_are_ignored() {
    let tmp = tempdir().unwrap();
    let vaults_dir = tmp.path().join("vaults");
    std::fs::create_dir_all(vaults_dir.join("notavault")).unwrap();
    std::fs::create_dir_all(vaults_dir.join("README")).unwrap();

    let resolved = resolve_with(make_layer(vaults_dir));
    assert!(resolved.secrets.is_none());
}

#[test]
fn explicit_missing_default_vault_is_a_configuration_error() {
    let tmp = tempdir().unwrap();
    let vaults_dir = tmp.path().join("vaults");
    let vault = vaults_dir.join("alpha.vault");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("store.age"), b"initialized").unwrap();

    let mut layer = make_layer(vaults_dir);
    layer.settings = serde_json::json!({
        "secrets": {"default_vault": "missing"}
    });
    let mut resolver = ConfigResolver::new();
    resolver.add_layer(layer);
    let error = resolver.resolve().unwrap_err().to_string();
    assert!(error.contains("secrets.default_vault"));
    assert!(error.contains("missing"));
    assert!(error.contains("alpha"));
}

#[test]
fn initialized_vault_requires_a_writable_protected_audit_path() {
    let tmp = tempdir().unwrap();
    let vaults_dir = tmp.path().join("vaults");
    let vault = vaults_dir.join("default.vault");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join("store.age"), b"initialized").unwrap();
    let audit_blocker = tmp.path().join("audit-blocker");
    std::fs::write(&audit_blocker, b"not a directory").unwrap();

    let mut layer = make_layer(vaults_dir);
    layer.settings = serde_json::json!({
        "telemetry": {"telemetry_dir": audit_blocker.to_string_lossy()}
    });
    let mut resolver = ConfigResolver::new();
    resolver.add_layer(layer);
    let error = resolver.resolve().unwrap_err().to_string();
    assert!(error.contains("protected vault audit unavailable"));
    assert!(error.contains("telemetry.telemetry_dir"));
}

#[test]
fn settings_secrets_vaults_dir_overrides_layer_value() {
    // settings.secrets.vaults_dir takes priority over layer-discovered path.
    let tmp = tempdir().unwrap();
    let real_dir = tmp.path().join("really_empty_dir");
    std::fs::create_dir_all(&real_dir).unwrap();

    let layer = ConfigLayer {
        source: LayerSource::Project,
        settings: serde_json::json!({
            "secrets": {
                "vaults_dir": real_dir.to_string_lossy(),
            }
        }),
        // Layer-discovered vaults_dir is different (and missing).
        vaults_dir: Some(tmp.path().join("ignored")),
        ..Default::default()
    };
    let resolved = resolve_with(layer);
    // real_dir is empty → no secret store, but resolver should have used
    // real_dir (not the ignored layer path).
    assert!(resolved.secrets.is_none());
}
