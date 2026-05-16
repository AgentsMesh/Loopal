use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use loopal_vault_api::{VaultError, VaultResult};
use secrecy::{ExposeSecret, SecretString};

use crate::identity::DiscoveredIdentity;
use crate::recipients::Recipients;
use crate::vault_io;

pub trait EditorAction {
    fn edit(&self, path: &Path) -> VaultResult<()>;
}

impl<F> EditorAction for F
where
    F: Fn(&Path) -> VaultResult<()>,
{
    fn edit(&self, path: &Path) -> VaultResult<()> {
        self(path)
    }
}

pub struct SystemEditor;

impl EditorAction for SystemEditor {
    fn edit(&self, path: &Path) -> VaultResult<()> {
        let raw = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let mut parts = raw.split_whitespace();
        let cmd = parts
            .next()
            .ok_or_else(|| VaultError::EditorFailed("EDITOR is empty; set $EDITOR".into()))?;
        let args: Vec<&str> = parts.collect();
        let status = Command::new(cmd)
            .args(&args)
            .arg(path)
            .status()
            .map_err(|e| VaultError::EditorFailed(format!("spawn {cmd}: {e}")))?;
        if !status.success() {
            return Err(VaultError::EditorFailed(format!(
                "editor {cmd} exited {status}"
            )));
        }
        Ok(())
    }
}

pub struct EditSession<'a> {
    pub vault_path: &'a Path,
    pub recipients_path: &'a Path,
    pub identity: &'a DiscoveredIdentity,
}

impl<'a> EditSession<'a> {
    pub fn run<E: EditorAction>(&self, editor: &E) -> VaultResult<()> {
        let secrets = self.read_current()?;
        let tmp = create_secure_tempfile(self.vault_path)?;
        write_plaintext_yaml(&tmp, &secrets)?;
        let result = (|| -> VaultResult<()> {
            editor.edit(&tmp)?;
            let new_yaml = fs::read_to_string(&tmp)
                .map_err(|e| VaultError::EditorFailed(format!("read tmp: {e}")))?;
            let new_secrets = parse_plaintext_yaml(&new_yaml)?;
            let recipients = Recipients::load(self.recipients_path)?;
            if recipients.is_empty() {
                return Err(VaultError::EncryptionFailed(
                    "no recipients configured at .age-recipients".into(),
                ));
            }
            vault_io::write_vault(self.vault_path, &recipients, &new_secrets)
        })();
        zero_and_remove(&tmp);
        result
    }

    fn read_current(&self) -> VaultResult<BTreeMap<String, SecretString>> {
        if !self.vault_path.exists() {
            return Ok(BTreeMap::new());
        }
        self.identity.ensure_usable()?;
        vault_io::read_vault(self.vault_path, &self.identity.identity)
    }
}

fn create_secure_tempfile(vault_path: &Path) -> VaultResult<PathBuf> {
    let parent = vault_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_backend)?;
    // reason: name matches `*.tmp.*` so the vault `.gitignore` excludes it,
    // preventing plaintext leakage if the editor process crashes mid-edit.
    let tmp = parent.join(format!("secrets-edit.tmp.{}.yaml", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp)
        .map_err(io_backend)?;
    file.write_all(b"").map_err(io_backend)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&tmp).map_err(io_backend)?.permissions();
        p.set_mode(0o600);
        fs::set_permissions(&tmp, p).map_err(io_backend)?;
    }
    Ok(tmp)
}

fn write_plaintext_yaml(path: &Path, secrets: &BTreeMap<String, SecretString>) -> VaultResult<()> {
    let raw: BTreeMap<String, &str> = secrets
        .iter()
        .map(|(k, v)| (k.clone(), v.expose_secret()))
        .collect();
    let yaml = if raw.is_empty() {
        String::from("# add entries like:\n# my_secret: value\n")
    } else {
        serde_yaml::to_string(&raw)
            .map_err(|e| VaultError::EncryptionFailed(format!("yaml: {e}")))?
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(io_backend)?;
    file.write_all(yaml.as_bytes()).map_err(io_backend)?;
    file.sync_all().map_err(io_backend)?;
    Ok(())
}

fn parse_plaintext_yaml(yaml: &str) -> VaultResult<BTreeMap<String, SecretString>> {
    let trimmed = yaml.trim();
    if trimmed.is_empty() {
        return Ok(BTreeMap::new());
    }
    let raw: BTreeMap<String, String> = serde_yaml::from_str(yaml)
        .map_err(|e| VaultError::DecryptionFailed(format!("yaml: {e}")))?;
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        vault_io::validate_secret_name(&k)?;
        out.insert(k, SecretString::from(v));
    }
    Ok(out)
}

fn zero_and_remove(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        let len = meta.len() as usize;
        if let Ok(mut f) = fs::OpenOptions::new().write(true).truncate(true).open(path) {
            let _ = f.write_all(&vec![0u8; len.min(64 * 1024)]);
            let _ = f.sync_all();
        }
    }
    let _ = fs::remove_file(path);
}

fn io_backend(e: std::io::Error) -> VaultError {
    VaultError::Backend(format!("io: {e}"))
}
