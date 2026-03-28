//! Credential persistence for MCP OAuth tokens.
//!
//! Stores tokens at `~/.loopal/oauth/<hash>.json` where hash is
//! SHA256(server_url). Tokens survive process restarts.

use std::path::PathBuf;

use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};

/// File-based credential store at `~/.loopal/oauth/`.
pub struct FileCredentialStore {
    path: PathBuf,
}

impl FileCredentialStore {
    pub fn new(server_url: &str) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        server_url.hash(&mut hasher);
        let hash = format!("{:016x}", hasher.finish());

        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".loopal/oauth");
        Self {
            path: dir.join(format!("{hash}.json")),
        }
    }
}

#[async_trait::async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        let creds: StoredCredentials = serde_json::from_str(&data)
            .map_err(|e| AuthError::InternalError(format!("parse credentials: {e}")))?;
        Ok(Some(creds))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
        }
        let data = serde_json::to_string_pretty(&credentials)
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        tokio::fs::write(&self.path, data)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        if self.path.exists() {
            tokio::fs::remove_file(&self.path)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
        }
        Ok(())
    }
}
