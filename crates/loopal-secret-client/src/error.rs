use std::path::PathBuf;

pub type SecretResult<T> = Result<T, SecretError>;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("vault not found for cwd {0}")]
    VaultNotFound(PathBuf),

    #[error("secret '{0}' not found")]
    SecretNotFound(String),

    #[error("permission denied: caller cwd not in vault tree")]
    PermissionDenied,

    #[error("decryption failed: {0}")]
    DecryptFailed(String),

    #[error("invalid secret name '{0}'; must match ^[a-z][a-z0-9_]*$")]
    InvalidName(String),

    #[error("template parse error: {0}")]
    TemplateParse(String),

    #[error("[hub_unavailable] IPC: {0}")]
    Ipc(String),
}
