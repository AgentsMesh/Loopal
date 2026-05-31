use std::path::PathBuf;
use std::sync::Arc;

use loopal_error::MemorySubsystemBootstrapError;

use crate::event_log::{EventLogWriter, fold_events, run_gc};
use crate::render::render_memory_md;
use crate::store::MemoryGraph;
use crate::sync::init::scan_directory;
use crate::sync::memory_index_path;
use crate::sync::watcher::{WatcherHandle, watch};

const INDEX_DB_NAME: &str = "memory.db";

pub struct MemorySubsystem {
    graph: Arc<MemoryGraph>,
    _watcher: Option<WatcherHandle>,
}

impl MemorySubsystem {
    pub fn new(graph: Arc<MemoryGraph>, watcher: Option<WatcherHandle>) -> Self {
        Self {
            graph,
            _watcher: watcher,
        }
    }

    pub fn graph(&self) -> Arc<MemoryGraph> {
        self.graph.clone()
    }

    pub async fn bootstrap(
        memory_dir: PathBuf,
        session_dir: PathBuf,
        events_dir: PathBuf,
        session_id: String,
        gc_compress_after_days: u32,
        gc_archive_after_days: u32,
    ) -> Result<Arc<Self>, MemorySubsystemBootstrapError> {
        let memory_dir = memory_dir
            .canonicalize()
            .map_err(MemorySubsystemBootstrapError::CanonicalizeMemoryDir)?;
        tokio::fs::create_dir_all(&session_dir)
            .await
            .map_err(MemorySubsystemBootstrapError::CreateSessionDir)?;
        if let Err(e) = tokio::fs::create_dir_all(&events_dir).await {
            tracing::warn!(error = %e, path = %events_dir.display(), "memory-events dir create failed; events will be skipped");
        }
        let db_path = session_dir.join(INDEX_DB_NAME);
        let mut graph = MemoryGraph::open(&db_path)?;

        if events_dir.exists() {
            let gc = run_gc(&events_dir, gc_compress_after_days, gc_archive_after_days);
            if gc.compressed + gc.archived + gc.errors > 0 {
                tracing::info!(
                    events_dir = %events_dir.display(),
                    compressed = gc.compressed,
                    archived = gc.archived,
                    errors = gc.errors,
                    "memory event log gc complete"
                );
            }
            let stats = fold_events(&events_dir);
            let stats_count = stats.len();
            graph.install_recall_stats(stats);
            tracing::info!(
                events_dir = %events_dir.display(),
                nodes_with_stats = stats_count,
                "memory event log fold complete"
            );
        }
        let event_log = Arc::new(EventLogWriter::new(events_dir.clone(), session_id));
        graph.set_event_log(event_log);

        let graph = Arc::new(graph);

        match scan_directory(&graph, &memory_dir).await {
            Ok(stats) => tracing::info!(
                files = stats.files_scanned,
                nodes = stats.nodes_indexed,
                edges = stats.edges_indexed,
                errors = stats.errors.len(),
                "memory graph initial scan complete"
            ),
            Err(e) => {
                tracing::warn!(error = %e, "memory initial scan failed; recall tool still registered")
            }
        }

        if let Ok(md) = render_memory_md(&graph).await {
            let path = memory_index_path(&memory_dir);
            if let Err(e) = tokio::fs::write(&path, md).await {
                tracing::warn!(error = %e, path = %path.display(), "failed to write rendered memory index");
            }
        }

        let watcher_handle = match watch(graph.clone(), memory_dir.clone()) {
            Ok(handle) => Some(handle),
            Err(e) => {
                tracing::warn!(error = %e, "memory watcher failed to start; init scan still effective");
                None
            }
        };

        Ok(Arc::new(Self::new(graph, watcher_handle)))
    }
}
