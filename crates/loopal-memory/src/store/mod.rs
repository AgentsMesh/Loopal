mod candidates;
mod connection;
mod edge_ops;
mod file_ops;
mod migrations;
mod node_ops;
pub(crate) mod queries_edge;
pub(crate) mod queries_files;
mod queries_fts;
mod queries_graph;
pub(crate) mod queries_node;
mod schema;
pub mod types;

use std::path::Path;
use std::sync::{Arc, RwLock};

use loopal_error::MemoryGraphError;

pub use connection::DbConnection;
pub use queries_fts::FtsHit;
pub use queries_graph::{BfsConfig, Subgraph};
pub use types::{EdgeKind, MemoryEdge, MemoryKind, MemoryNode, Provenance};

use crate::event_log::{EventKind, EventLogWriter, RecallStats, RecallStatsMap};

#[derive(Clone)]
pub struct MemoryGraph {
    pub(crate) db: DbConnection,
    pub(crate) recall_stats: Arc<RwLock<RecallStatsMap>>,
    pub(crate) event_log: Option<Arc<EventLogWriter>>,
}

impl MemoryGraph {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MemoryGraphError> {
        Ok(Self {
            db: DbConnection::open(path)?,
            recall_stats: Arc::new(RwLock::new(RecallStatsMap::new())),
            event_log: None,
        })
    }

    pub fn in_memory() -> Result<Self, MemoryGraphError> {
        Ok(Self {
            db: DbConnection::in_memory()?,
            recall_stats: Arc::new(RwLock::new(RecallStatsMap::new())),
            event_log: None,
        })
    }

    pub fn set_event_log(&mut self, log: Arc<EventLogWriter>) {
        self.event_log = Some(log);
    }

    pub fn install_recall_stats(&mut self, stats: RecallStatsMap) {
        if let Ok(mut guard) = self.recall_stats.write() {
            *guard = stats;
        }
    }

    pub fn recall_stats_snapshot(&self, slug: &str) -> Option<RecallStats> {
        self.recall_stats.read().ok()?.get(slug).cloned()
    }

    pub fn record_event(&self, kind: EventKind) {
        let Some(log) = &self.event_log else {
            return;
        };
        let ev = log.append(kind);
        if let Ok(mut guard) = self.recall_stats.write()
            && let Some(slug) = ev.node_slug()
        {
            guard.entry(slug.to_string()).or_default().fold_event(&ev);
        }
    }

    pub async fn search(
        &self,
        query: &str,
        kind: Option<MemoryKind>,
        limit: usize,
    ) -> Result<Vec<FtsHit>, MemoryGraphError> {
        let q = query.to_string();
        self.db
            .with_conn(move |c| queries_fts::search(c, &q, kind, limit))
            .await
    }

    pub async fn bfs(
        &self,
        start_ids: &[String],
        config: BfsConfig,
    ) -> Result<Subgraph, MemoryGraphError> {
        let ids = start_ids.to_vec();
        self.db
            .with_conn(move |c| queries_graph::bfs_bidirectional(c, &ids, &config))
            .await
    }

    pub async fn find_high_impact(
        &self,
        limit: usize,
    ) -> Result<Vec<(MemoryNode, usize)>, MemoryGraphError> {
        self.db
            .with_conn(move |c| candidates::high_impact(c, limit))
            .await
    }

    pub async fn find_expired(&self) -> Result<Vec<MemoryNode>, MemoryGraphError> {
        let now = chrono::Utc::now().timestamp_millis();
        self.db
            .with_conn(move |c| candidates::expired(c, now))
            .await
    }

    pub async fn find_conflicting_pairs(&self) -> Result<Vec<(String, String)>, MemoryGraphError> {
        self.db.with_conn(candidates::conflicting_pairs).await
    }
}
