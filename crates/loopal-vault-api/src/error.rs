use std::path::PathBuf;

use crate::AuditError;

pub type VaultResult<T> = Result<T, VaultError>;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault file not found at {0}")]
    NotFound(PathBuf),

    #[error("no identity discovered (looked in ~/.ssh)")]
    IdentityMissing,

    #[error("identity {0} has insecure permissions; expected 0600")]
    InsecureIdentityPermissions(PathBuf),

    #[error("identity {path} is passphrase-protected and no agent socket available")]
    PassphraseProtected { path: PathBuf },

    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("invalid secret name '{0}'; must match ^[a-z][a-z0-9_]*$")]
    InvalidSecretName(String),

    #[error("invalid vault name '{0}'; must match ^[a-z][a-z0-9_-]*$")]
    InvalidVaultName(String),

    #[error("secret '{0}' not found in any vault")]
    SecretNotFound(String),

    #[error("invalid recipient line: {0}")]
    InvalidRecipient(String),

    #[error("recipient label '{0}' not found")]
    RecipientNotFound(String),

    #[error("editor command failed: {0}")]
    EditorFailed(String),

    #[error("protected audit failed: {0}")]
    Audit(#[from] AuditError),

    #[error("backend: {0}")]
    Backend(String),
}
