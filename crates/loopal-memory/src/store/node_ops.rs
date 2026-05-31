use crate::store::{MemoryGraph, MemoryKind, MemoryNode, queries_node};
use loopal_error::MemoryGraphError;

impl MemoryGraph {
    pub async fn upsert_node(&self, node: MemoryNode) -> Result<(), MemoryGraphError> {
        self.db.with_conn(|c| queries_node::upsert(c, &node)).await
    }

    pub async fn get_node(&self, id: &str) -> Result<Option<MemoryNode>, MemoryGraphError> {
        let id = id.to_string();
        self.db.with_conn(move |c| queries_node::get(c, &id)).await
    }

    pub async fn get_nodes(&self, ids: &[String]) -> Result<Vec<MemoryNode>, MemoryGraphError> {
        let ids = ids.to_vec();
        self.db
            .with_conn(move |c| queries_node::get_many(c, &ids))
            .await
    }

    pub async fn list_nodes(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<MemoryNode>, MemoryGraphError> {
        self.db
            .with_conn(move |c| queries_node::list(c, limit))
            .await
    }

    pub async fn list_nodes_by_kind(
        &self,
        kind: MemoryKind,
    ) -> Result<Vec<MemoryNode>, MemoryGraphError> {
        self.db
            .with_conn(move |c| queries_node::list_by_kind(c, kind))
            .await
    }

    pub async fn delete_node(&self, id: &str) -> Result<bool, MemoryGraphError> {
        let id = id.to_string();
        self.db
            .with_conn(move |c| queries_node::delete(c, &id))
            .await
    }

    pub async fn rename_node(
        &self,
        old_id: &str,
        new_id: &str,
        new_file_path: &str,
    ) -> Result<bool, MemoryGraphError> {
        let old_id = old_id.to_string();
        let new_id = new_id.to_string();
        let new_file_path = new_file_path.to_string();
        self.db
            .with_conn_mut(move |conn| {
                let tx = conn.transaction()?;
                let renamed = queries_node::rename(&tx, &old_id, &new_id, &new_file_path)?;
                tx.commit()?;
                Ok(renamed)
            })
            .await
    }

    pub async fn find_node_by_path(
        &self,
        path: &str,
    ) -> Result<Option<MemoryNode>, MemoryGraphError> {
        let path = path.to_string();
        self.db
            .with_conn(move |c| queries_node::find_by_file_path(c, &path))
            .await
    }

    pub async fn node_count(&self) -> Result<usize, MemoryGraphError> {
        self.db.with_conn(queries_node::count).await
    }
}
