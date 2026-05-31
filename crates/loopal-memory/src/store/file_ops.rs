use crate::store::MemoryGraph;
use crate::store::queries_files::{self, FileCacheEntry};
use loopal_error::MemoryGraphError;

impl MemoryGraph {
    pub async fn get_file_cache(
        &self,
        path: &str,
    ) -> Result<Option<FileCacheEntry>, MemoryGraphError> {
        let path = path.to_string();
        self.db
            .with_conn(move |c| queries_files::get(c, &path))
            .await
    }

    pub async fn upsert_file_cache(
        &self,
        path: &str,
        content_hash: &str,
        size: i64,
        modified_at: i64,
        indexed_at: i64,
    ) -> Result<(), MemoryGraphError> {
        let path = path.to_string();
        let content_hash = content_hash.to_string();
        self.db
            .with_conn(move |c| {
                queries_files::upsert(c, &path, &content_hash, size, modified_at, indexed_at)
            })
            .await
    }

    pub async fn delete_file_cache(&self, path: &str) -> Result<bool, MemoryGraphError> {
        let path = path.to_string();
        self.db
            .with_conn(move |c| queries_files::delete(c, &path))
            .await
    }
}
