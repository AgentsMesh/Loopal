use std::sync::Arc;

#[derive(Clone)]
pub struct DecisionContext {
    cwd: Arc<str>,
    recent: Arc<tokio::sync::RwLock<String>>,
}

impl DecisionContext {
    pub fn with_cwd(cwd: impl Into<String>) -> Self {
        Self {
            cwd: Arc::from(cwd.into()),
            recent: Arc::new(tokio::sync::RwLock::new(String::new())),
        }
    }

    pub async fn set_recent(&self, recent: String) {
        *self.recent.write().await = recent;
    }

    pub async fn recent(&self) -> String {
        self.recent.read().await.clone()
    }

    pub fn cwd(&self) -> &str {
        &self.cwd
    }
}
