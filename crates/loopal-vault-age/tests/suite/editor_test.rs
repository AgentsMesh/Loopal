use std::sync::Arc;
use tempfile::tempdir;

use loopal_vault_age::{AgeVault, EditSession, EditorAction, Recipients};
use loopal_vault_api::{Vault, VaultError};
use secrecy::{ExposeSecret, SecretString};

use crate::ssh_fixtures as fx;

const PUBKEY_ALICE: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";

#[cfg(unix)]
fn write_key(path: &std::path::Path, content: &str, mode: u32) {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    let mut p = fs::metadata(path).unwrap().permissions();
    p.set_mode(mode);
    fs::set_permissions(path, p).unwrap();
}

#[cfg(not(unix))]
fn write_key(path: &std::path::Path, content: &str, _mode: u32) {
    std::fs::write(path, content).unwrap();
}

struct EditorScript<F: Fn(&std::path::Path) -> std::io::Result<()> + Send + Sync> {
    action: F,
}

impl<F: Fn(&std::path::Path) -> std::io::Result<()> + Send + Sync> EditorAction
    for EditorScript<F>
{
    fn edit(&self, path: &std::path::Path) -> loopal_vault_api::VaultResult<()> {
        (self.action)(path)
            .map_err(|e| VaultError::EditorFailed(format!("test editor io error: {e}")))
    }
}

fn build_session_dir() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    Arc<loopal_vault_age::DiscoveredIdentity>,
) {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    write_key(&key_path, fx::ED25519_UNENCRYPTED, 0o600);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());

    let rec_path = dir.path().join(".age-recipients");
    let mut rec = Recipients::new();
    rec.add_line(PUBKEY_ALICE).unwrap();
    rec.write(&rec_path).unwrap();

    let vault = dir.path().join("secrets.yaml.age");
    (dir, vault, rec_path, identity)
}

#[tokio::test]
async fn empty_vault_edit_writes_new_secret() {
    let (_dir, vault, recipients, identity) = build_session_dir();
    let editor = EditorScript {
        action: |p: &std::path::Path| std::fs::write(p, "added_key: this-is-a-token\n"),
    };
    let session = EditSession {
        vault_path: &vault,
        recipients_path: &recipients,
        identity: &identity,
    };
    session.run(&editor).unwrap();

    let store = AgeVault::new(vault, recipients, identity);
    let v = store.get("added_key").await.unwrap();
    assert_eq!(v.expose_secret(), "this-is-a-token");
}

#[tokio::test]
async fn edit_existing_vault_modifies_value() {
    let (_dir, vault, recipients, identity) = build_session_dir();

    let initial = EditorScript {
        action: |p: &std::path::Path| std::fs::write(p, "k1: first-value\n"),
    };
    EditSession {
        vault_path: &vault,
        recipients_path: &recipients,
        identity: &identity,
    }
    .run(&initial)
    .unwrap();

    let modify = EditorScript {
        action: |p: &std::path::Path| std::fs::write(p, "k1: second-value\n"),
    };
    EditSession {
        vault_path: &vault,
        recipients_path: &recipients,
        identity: &identity,
    }
    .run(&modify)
    .unwrap();

    let store = AgeVault::new(vault, recipients, identity);
    assert_eq!(
        store.get("k1").await.unwrap().expose_secret(),
        "second-value"
    );
}

#[tokio::test]
async fn edit_can_delete_by_clearing_entry() {
    let (_dir, vault, recipients, identity) = build_session_dir();
    let store = AgeVault::new(vault.clone(), recipients.clone(), identity.clone());
    store
        .put("kx", SecretString::from("12345678"))
        .await
        .unwrap();

    let clearing = EditorScript {
        action: |p: &std::path::Path| std::fs::write(p, ""),
    };
    EditSession {
        vault_path: &vault,
        recipients_path: &recipients,
        identity: &identity,
    }
    .run(&clearing)
    .unwrap();

    let store2 = AgeVault::new(vault, recipients, identity);
    assert!(store2.list_names().await.is_empty());
}

#[tokio::test]
async fn invalid_name_in_edit_rejected_and_vault_unchanged() {
    let (_dir, vault, recipients, identity) = build_session_dir();
    let store = AgeVault::new(vault.clone(), recipients.clone(), identity.clone());
    store
        .put("ok_name", SecretString::from("preserved_v"))
        .await
        .unwrap();

    let bad = EditorScript {
        action: |p: &std::path::Path| std::fs::write(p, "BadName: x\n"),
    };
    let result = EditSession {
        vault_path: &vault,
        recipients_path: &recipients,
        identity: &identity,
    }
    .run(&bad);
    assert!(matches!(result, Err(VaultError::InvalidSecretName(_))));

    let store2 = AgeVault::new(vault, recipients, identity);
    assert_eq!(
        store2.get("ok_name").await.unwrap().expose_secret(),
        "preserved_v"
    );
}

#[tokio::test]
async fn tempfile_removed_after_edit() {
    let (dir, vault, recipients, identity) = build_session_dir();
    let editor = EditorScript {
        action: |p: &std::path::Path| std::fs::write(p, "k: 12345678\n"),
    };
    EditSession {
        vault_path: &vault,
        recipients_path: &recipients,
        identity: &identity,
    }
    .run(&editor)
    .unwrap();

    let stray: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name();
            n.to_string_lossy().starts_with("secrets-edit.tmp.")
        })
        .collect();
    assert!(stray.is_empty(), "tempfile should be cleaned up");
}
