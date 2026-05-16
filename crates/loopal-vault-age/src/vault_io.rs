use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use loopal_vault_api::{VaultError, VaultResult};
use once_cell::sync::Lazy;
use regex::Regex;
use secrecy::{ExposeSecret, SecretString};

use crate::recipients::Recipients;

static NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z][a-z0-9_]*$").expect("valid name regex"));

pub fn validate_secret_name(name: &str) -> VaultResult<()> {
    if NAME_RE.is_match(name) {
        Ok(())
    } else {
        Err(VaultError::InvalidSecretName(name.to_string()))
    }
}

pub fn read_vault(
    path: &Path,
    identity: &dyn age::Identity,
) -> VaultResult<BTreeMap<String, SecretString>> {
    if !path.exists() {
        return Err(VaultError::NotFound(path.to_path_buf()));
    }
    let ciphertext = fs::read(path).map_err(io_backend)?;
    let decryptor = age::Decryptor::new(&ciphertext[..])
        .map_err(|e| VaultError::DecryptionFailed(format!("header: {e}")))?;
    let mut reader = match decryptor {
        age::Decryptor::Recipients(d) => d
            .decrypt(std::iter::once(identity))
            .map_err(|e| VaultError::DecryptionFailed(format!("decrypt: {e}")))?,
        age::Decryptor::Passphrase(_) => {
            return Err(VaultError::DecryptionFailed(
                "vault uses passphrase mode; recipients required".into(),
            ));
        }
    };
    let mut yaml = String::new();
    reader
        .read_to_string(&mut yaml)
        .map_err(|e| VaultError::DecryptionFailed(format!("read: {e}")))?;
    if yaml.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    let raw: BTreeMap<String, String> = serde_yaml::from_str(&yaml)
        .map_err(|e| VaultError::DecryptionFailed(format!("yaml: {e}")))?;
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        validate_secret_name(&k)?;
        out.insert(k, SecretString::from(v));
    }
    Ok(out)
}

pub fn write_vault(
    path: &Path,
    recipients: &Recipients,
    secrets: &BTreeMap<String, SecretString>,
) -> VaultResult<()> {
    if recipients.is_empty() {
        return Err(VaultError::EncryptionFailed(
            "no recipients configured; add at least one before writing".into(),
        ));
    }
    for k in secrets.keys() {
        validate_secret_name(k)?;
    }
    let raw: BTreeMap<String, &str> = secrets
        .iter()
        .map(|(k, v)| (k.clone(), v.expose_secret()))
        .collect();
    let yaml = serde_yaml::to_string(&raw)
        .map_err(|e| VaultError::EncryptionFailed(format!("yaml: {e}")))?;
    let recipients_vec = recipients.recipients_for_encryption();
    let encryptor = age::Encryptor::with_recipients(recipients_vec)
        .ok_or_else(|| VaultError::EncryptionFailed("could not construct encryptor".into()))?;
    let mut ciphertext: Vec<u8> = Vec::new();
    {
        let mut writer = encryptor
            .wrap_output(&mut ciphertext)
            .map_err(|e| VaultError::EncryptionFailed(format!("wrap: {e}")))?;
        writer
            .write_all(yaml.as_bytes())
            .map_err(|e| VaultError::EncryptionFailed(format!("write: {e}")))?;
        writer
            .finish()
            .map_err(|e| VaultError::EncryptionFailed(format!("finish: {e}")))?;
    }
    atomic_write(path, &ciphertext)?;
    Ok(())
}

fn io_backend(e: std::io::Error) -> VaultError {
    VaultError::Backend(format!("io: {e}"))
}

fn atomic_write(path: &Path, data: &[u8]) -> VaultResult<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_backend)?;
    let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(io_backend)?;
        file.write_all(data).map_err(io_backend)?;
        file.sync_all().map_err(io_backend)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&tmp_path).map_err(io_backend)?.permissions();
        p.set_mode(0o600);
        fs::set_permissions(&tmp_path, p).map_err(io_backend)?;
    }
    fs::rename(&tmp_path, path).map_err(io_backend)?;
    Ok(())
}

const LOCK_RETRY_COUNT: u32 = 50;
const LOCK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// RAII guard for an exclusive cross-process lock on the store.
pub struct StoreLock {
    path: std::path::PathBuf,
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub async fn acquire_store_lock(store_path: &Path) -> VaultResult<StoreLock> {
    let parent = store_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_backend)?;
    let lock_path = store_path.with_extension(format!(
        "{}.lock",
        store_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("vault")
    ));
    for _ in 0..LOCK_RETRY_COUNT {
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path)
        {
            Ok(file) => {
                let _ = writeln!(&file, "pid={}", std::process::id());
                return Ok(StoreLock { path: lock_path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                tokio::time::sleep(LOCK_RETRY_DELAY).await;
            }
            Err(e) => return Err(io_backend(e)),
        }
    }
    Err(VaultError::EncryptionFailed(format!(
        "could not acquire store lock at {} after {} retries",
        lock_path.display(),
        LOCK_RETRY_COUNT
    )))
}
