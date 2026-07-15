//! Credential persistence for MCP OAuth tokens.

use std::path::PathBuf;

use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};

use super::store_io::{secure_read, secure_write};

pub struct FileCredentialStore {
    path: Option<PathBuf>,
}

impl FileCredentialStore {
    pub fn new(server_url: &str) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        server_url.hash(&mut hasher);
        let hash = format!("{:016x}", hasher.finish());
        let path =
            dirs::home_dir().map(|home| home.join(".loopal/oauth").join(format!("{hash}.json")));
        Self { path }
    }
}

#[async_trait::async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| fixed_error("OAuth credential storage unavailable"))?;
        let data = tokio::task::spawn_blocking(move || secure_read(&path))
            .await
            .map_err(|_| fixed_error("OAuth credential read failed"))?
            .map_err(|_| fixed_error("OAuth credential read failed"))?;
        let Some(data) = data else {
            return Ok(None);
        };
        serde_json::from_str(&data)
            .map(Some)
            .map_err(|_| fixed_error("OAuth credential parse failed"))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let data = serde_json::to_vec_pretty(&credentials)
            .map_err(|_| fixed_error("OAuth credential serialization failed"))?;
        let path = self
            .path
            .clone()
            .ok_or_else(|| fixed_error("OAuth credential storage unavailable"))?;
        tokio::task::spawn_blocking(move || secure_write(&path, &data))
            .await
            .map_err(|_| fixed_error("OAuth credential write failed"))?
            .map_err(|_| fixed_error("OAuth credential write failed"))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| fixed_error("OAuth credential storage unavailable"))?;
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(fixed_error("OAuth credential clear failed")),
        }
    }
}

fn fixed_error(message: &'static str) -> AuthError {
    AuthError::InternalError(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unavailable_home_fails_closed_without_paths_or_tokens() {
        let store = FileCredentialStore { path: None };
        let error = match store.load().await {
            Err(error) => error,
            Ok(_) => panic!("missing HOME must disable OAuth persistence"),
        };
        let display = format!("{error}");
        assert!(display.contains("storage unavailable"));
        assert!(!display.contains("oauth-token-secret-marker"));
        assert!(!format!("{error:?}").contains("oauth-token-secret-marker"));
    }
}
