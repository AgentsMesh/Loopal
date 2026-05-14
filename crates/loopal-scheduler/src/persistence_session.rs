use async_trait::async_trait;

use crate::persistence::{PersistError, PersistedTask};

#[async_trait]
pub trait SessionScopedCronStorage: Send + Sync {
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError>;

    /// Replace the stored set for `session_id` with `tasks`. Must be
    /// atomic — readers see either the old or the new state, never
    /// partial.
    async fn save_all(&self, session_id: &str, tasks: &[PersistedTask])
    -> Result<(), PersistError>;
}
