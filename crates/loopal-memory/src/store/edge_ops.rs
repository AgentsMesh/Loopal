use crate::store::{MemoryEdge, MemoryGraph, Provenance, queries_edge};
use loopal_error::MemoryGraphError;

impl MemoryGraph {
    pub async fn insert_edge(&self, edge: MemoryEdge) -> Result<i64, MemoryGraphError> {
        self.db
            .with_conn(move |c| queries_edge::insert(c, &edge))
            .await
    }

    pub async fn get_outgoing_edges(
        &self,
        src_id: &str,
    ) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
        let id = src_id.to_string();
        self.db
            .with_conn(move |c| queries_edge::get_outgoing(c, &id))
            .await
    }

    pub async fn get_incoming_edges(
        &self,
        dst_id: &str,
    ) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
        let id = dst_id.to_string();
        self.db
            .with_conn(move |c| queries_edge::get_incoming(c, &id))
            .await
    }

    pub async fn count_incoming(&self, dst_id: &str) -> Result<usize, MemoryGraphError> {
        let id = dst_id.to_string();
        self.db
            .with_conn(move |c| queries_edge::count_incoming(c, &id))
            .await
    }

    pub async fn count_incoming_all(
        &self,
    ) -> Result<std::collections::HashMap<String, usize>, MemoryGraphError> {
        self.db.with_conn(queries_edge::count_incoming_all).await
    }

    pub async fn list_edges_by_provenance(
        &self,
        prov: Provenance,
    ) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
        self.db
            .with_conn(move |c| queries_edge::list_by_provenance(c, prov))
            .await
    }

    pub async fn delete_edges_for_node(&self, node_id: &str) -> Result<usize, MemoryGraphError> {
        let id = node_id.to_string();
        self.db
            .with_conn(move |c| queries_edge::delete_by_node(c, &id))
            .await
    }

    pub async fn delete_outgoing_edges(&self, src_id: &str) -> Result<usize, MemoryGraphError> {
        let id = src_id.to_string();
        self.db
            .with_conn(move |c| queries_edge::delete_outgoing(c, &id))
            .await
    }

    pub async fn delete_edges_by_provenance(
        &self,
        prov: Provenance,
    ) -> Result<usize, MemoryGraphError> {
        self.db
            .with_conn(move |c| queries_edge::delete_by_provenance(c, prov))
            .await
    }

    pub async fn edge_count(&self) -> Result<usize, MemoryGraphError> {
        self.db.with_conn(queries_edge::count).await
    }
}
